package app.streamsound

import android.annotation.SuppressLint
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioPlaybackCaptureConfiguration
import android.media.AudioRecord
import android.media.AudioTrack
import android.media.MediaRecorder
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager
import android.os.Process
import android.util.Log
import kotlin.concurrent.thread
import kotlin.math.max

/**
 * Keeps streaming alive with the screen off. Owns the engine, the speaker
 * thread (pulls mixed audio from the engine) and the capture thread (pushes
 * system/app/mic audio into the engine).
 */
class StreamService : Service() {

    companion object {
        private const val TAG = "StreamSound"
        private const val CHANNEL_ID = "stream"
        private const val NOTIF_ID = 1

        const val ACTION_SEND = "app.streamsound.SEND"
        const val ACTION_STOP_SEND = "app.streamsound.STOP_SEND"
        const val ACTION_QUIT = "app.streamsound.QUIT"
        const val EXTRA_MODE = "mode" // "system", "app" or "mic"
        const val EXTRA_UID = "uid"
        const val EXTRA_DESTS = "dests"
        const val EXTRA_RESULT_CODE = "resultCode"
        const val EXTRA_RESULT_DATA = "resultData"

        const val CAPTURE_RATE = 48000

        @Volatile
        var instance: StreamService? = null
            private set
    }

    @Volatile
    var handle: Long = 0L
        private set

    @Volatile
    private var running = false

    @Volatile
    private var capturing = false
    private var playbackThread: Thread? = null
    private var captureThread: Thread? = null
    private var projection: MediaProjection? = null
    private var multicastLock: WifiManager.MulticastLock? = null
    private var wifiLock: WifiManager.WifiLock? = null
    private var wakeLock: PowerManager.WakeLock? = null

    @Volatile
    var lastError: String = ""
        private set

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        createChannel()
        goForeground(withProjection = false, withMic = false)

        val name = (Build.MODEL ?: "Android").ifBlank { "Android" }
        handle = Native.create(name, 30f)
        val err = Native.startReceiving(handle)
        if (err.isNotEmpty()) lastError = err

        acquireLocks()
        running = true
        playbackThread = thread(name = "ssnd-playback") { playbackLoop() }
        instance = this
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_SEND -> startSending(intent)
            ACTION_STOP_SEND -> stopSending()
            ACTION_QUIT -> {
                stopSelf()
                return START_NOT_STICKY
            }
        }
        return START_STICKY
    }

    override fun onDestroy() {
        instance = null
        stopSending()
        running = false
        playbackThread?.join(1000)
        releaseLocks()
        val h = handle
        handle = 0L
        if (h != 0L) Native.destroy(h)
        super.onDestroy()
    }

    // ---- foreground + notification ---------------------------------------

    private fun createChannel() {
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(
            NotificationChannel(CHANNEL_ID, "Stream Sound", NotificationManager.IMPORTANCE_LOW)
        )
    }

    private fun notification(text: String): Notification {
        val open = PendingIntent.getActivity(
            this, 0, Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )
        return Notification.Builder(this, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_notif)
            .setContentTitle("Stream Sound")
            .setContentText(text)
            .setContentIntent(open)
            .setOngoing(true)
            .build()
    }

    private fun goForeground(withProjection: Boolean, withMic: Boolean) {
        var types = ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK
        if (withProjection) types = types or ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION
        if (withMic && Build.VERSION.SDK_INT >= 30) types = types or ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE
        val text = if (withProjection || withMic) "กำลังส่งและรับเสียง" else "พร้อมรับเสียง"
        try {
            startForeground(NOTIF_ID, notification(text), types)
        } catch (e: Exception) {
            Log.e(TAG, "startForeground failed", e)
            lastError = e.message ?: "startForeground failed"
        }
    }

    // ---- locks: keep Wi-Fi fast and the CPU awake while streaming ---------

    private fun acquireLocks() {
        try {
            val wifi = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
            multicastLock = wifi.createMulticastLock("ssnd-discovery").apply {
                setReferenceCounted(false); acquire()
            }
            @Suppress("DEPRECATION")
            val mode = if (Build.VERSION.SDK_INT >= 29) WifiManager.WIFI_MODE_FULL_LOW_LATENCY
            else WifiManager.WIFI_MODE_FULL_HIGH_PERF
            wifiLock = wifi.createWifiLock(mode, "ssnd-lowlatency").apply {
                setReferenceCounted(false); acquire()
            }
            val pm = getSystemService(Context.POWER_SERVICE) as PowerManager
            wakeLock = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "StreamSound:audio").apply {
                setReferenceCounted(false); acquire()
            }
        } catch (e: Exception) {
            Log.w(TAG, "lock failed", e)
        }
    }

    private fun releaseLocks() {
        try { multicastLock?.release() } catch (_: Exception) {}
        try { wifiLock?.release() } catch (_: Exception) {}
        try { wakeLock?.release() } catch (_: Exception) {}
    }

    // ---- speaker ------------------------------------------------------------

    private fun playbackLoop() {
        Process.setThreadPriority(Process.THREAD_PRIORITY_URGENT_AUDIO)
        while (running) {
            var track: AudioTrack? = null
            try {
                val am = getSystemService(Context.AUDIO_SERVICE) as AudioManager
                val rate = am.getProperty(AudioManager.PROPERTY_OUTPUT_SAMPLE_RATE)?.toIntOrNull() ?: 48000
                val framesPerBuffer = am.getProperty(AudioManager.PROPERTY_OUTPUT_FRAMES_PER_BUFFER)?.toIntOrNull() ?: 240
                val frames = max(framesPerBuffer, rate / 200)
                val minBuf = AudioTrack.getMinBufferSize(rate, AudioFormat.CHANNEL_OUT_STEREO, AudioFormat.ENCODING_PCM_FLOAT)
                track = AudioTrack.Builder()
                    .setAudioAttributes(
                        AudioAttributes.Builder()
                            .setUsage(AudioAttributes.USAGE_MEDIA)
                            .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                            .build()
                    )
                    .setAudioFormat(
                        AudioFormat.Builder()
                            .setEncoding(AudioFormat.ENCODING_PCM_FLOAT)
                            .setSampleRate(rate)
                            .setChannelMask(AudioFormat.CHANNEL_OUT_STEREO)
                            .build()
                    )
                    .setBufferSizeInBytes(max(minBuf, frames * 2 * 4 * 2))
                    .setPerformanceMode(AudioTrack.PERFORMANCE_MODE_LOW_LATENCY)
                    .setTransferMode(AudioTrack.MODE_STREAM)
                    .build()
                track.play()
                val buf = FloatArray(frames * 2)
                while (running) {
                    Native.render(handle, buf, frames, rate, 2)
                    val n = track.write(buf, 0, buf.size, AudioTrack.WRITE_BLOCKING)
                    if (n < 0) throw IllegalStateException("AudioTrack write error $n")
                }
            } catch (e: Exception) {
                // Output changed (headphones, Bluetooth) or failed: rebuild it.
                Log.w(TAG, "playback restarting", e)
                try { Thread.sleep(300) } catch (_: InterruptedException) {}
            } finally {
                try { track?.stop() } catch (_: Exception) {}
                try { track?.release() } catch (_: Exception) {}
            }
        }
    }

    // ---- capture ------------------------------------------------------------

    private fun startSending(intent: Intent) {
        stopSending()
        val mode = intent.getStringExtra(EXTRA_MODE) ?: "system"
        val uid = intent.getIntExtra(EXTRA_UID, -1)
        val dests = intent.getStringExtra(EXTRA_DESTS) ?: ""

        if (mode == "mic") {
            goForeground(withProjection = false, withMic = true)
        } else {
            goForeground(withProjection = true, withMic = false)
            val code = intent.getIntExtra(EXTRA_RESULT_CODE, 0)
            @Suppress("DEPRECATION")
            val data: Intent? = if (Build.VERSION.SDK_INT >= 33)
                intent.getParcelableExtra(EXTRA_RESULT_DATA, Intent::class.java)
            else intent.getParcelableExtra(EXTRA_RESULT_DATA)
            if (data == null) {
                lastError = "ไม่ได้รับอนุญาตให้จับเสียง"
                goForeground(withProjection = false, withMic = false)
                return
            }
            try {
                val mpm = getSystemService(Context.MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
                val p = mpm.getMediaProjection(code, data)
                p.registerCallback(object : MediaProjection.Callback() {
                    override fun onStop() {
                        stopSending()
                    }
                }, Handler(Looper.getMainLooper()))
                projection = p
            } catch (e: Exception) {
                lastError = e.message ?: "MediaProjection failed"
                goForeground(withProjection = false, withMic = false)
                return
            }
        }

        val err = Native.startSending(handle, dests)
        if (err.isNotEmpty()) {
            lastError = err
            stopSending()
            return
        }
        lastError = ""
        capturing = true
        captureThread = thread(name = "ssnd-capture") { captureLoop(mode, uid) }
    }

    fun stopSending() {
        capturing = false
        captureThread?.let { if (it != Thread.currentThread()) it.join(1000) }
        captureThread = null
        try { projection?.stop() } catch (_: Exception) {}
        projection = null
        if (handle != 0L) Native.stopSending(handle)
        if (running) goForeground(withProjection = false, withMic = false)
    }

    @SuppressLint("MissingPermission")
    private fun buildRecorder(mode: String, uid: Int): Pair<AudioRecord, Int> {
        val channels = if (mode == "mic") 1 else 2
        val mask = if (channels == 1) AudioFormat.CHANNEL_IN_MONO else AudioFormat.CHANNEL_IN_STEREO
        val format = AudioFormat.Builder()
            .setEncoding(AudioFormat.ENCODING_PCM_FLOAT)
            .setSampleRate(CAPTURE_RATE)
            .setChannelMask(mask)
            .build()
        val minBuf = AudioRecord.getMinBufferSize(CAPTURE_RATE, mask, AudioFormat.ENCODING_PCM_FLOAT)
        val bufBytes = max(minBuf, CAPTURE_RATE / 50 * channels * 4)
        val builder = AudioRecord.Builder().setAudioFormat(format).setBufferSizeInBytes(bufBytes)
        if (mode == "mic") {
            builder.setAudioSource(MediaRecorder.AudioSource.MIC)
        } else {
            val p = projection ?: throw IllegalStateException("no MediaProjection")
            val cfg = AudioPlaybackCaptureConfiguration.Builder(p)
            if (mode == "app" && uid >= 0) {
                cfg.addMatchingUid(uid)
            } else {
                cfg.addMatchingUsage(AudioAttributes.USAGE_MEDIA)
                cfg.addMatchingUsage(AudioAttributes.USAGE_GAME)
                cfg.addMatchingUsage(AudioAttributes.USAGE_UNKNOWN)
            }
            builder.setAudioPlaybackCaptureConfig(cfg.build())
        }
        return Pair(builder.build(), channels)
    }

    private fun captureLoop(mode: String, uid: Int) {
        Process.setThreadPriority(Process.THREAD_PRIORITY_URGENT_AUDIO)
        while (capturing) {
            var rec: AudioRecord? = null
            try {
                val (r, channels) = buildRecorder(mode, uid)
                rec = r
                if (r.state != AudioRecord.STATE_INITIALIZED) throw IllegalStateException("AudioRecord not initialized")
                r.startRecording()
                val buf = FloatArray(CAPTURE_RATE / 200 * channels) // 5 ms
                while (capturing) {
                    val n = r.read(buf, 0, buf.size, AudioRecord.READ_BLOCKING)
                    if (n > 0) Native.pushCapture(handle, buf, n, CAPTURE_RATE, channels)
                    else if (n < 0) throw IllegalStateException("AudioRecord read error $n")
                }
            } catch (e: Exception) {
                Log.w(TAG, "capture restarting", e)
                lastError = e.message ?: "capture error"
                try { Thread.sleep(500) } catch (_: InterruptedException) {}
            } finally {
                try { rec?.stop() } catch (_: Exception) {}
                try { rec?.release() } catch (_: Exception) {}
            }
        }
    }
}
