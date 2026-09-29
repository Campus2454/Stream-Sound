package app.streamsound

import android.Manifest
import android.app.Activity
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.SharedPreferences
import android.content.pm.PackageManager
import android.media.projection.MediaProjectionManager
import android.os.Build
import android.os.Bundle
import android.os.Process
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import app.streamsound.ui.AppInfo
import app.streamsound.ui.AppUi
import app.streamsound.ui.EngineState
import app.streamsound.ui.Host
import app.streamsound.ui.PeerInfo
import app.streamsound.ui.Store
import app.streamsound.ui.StreamInfo
import app.streamsound.ui.UiModel
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.File

/** Settings kept in SharedPreferences; a bad or old value falls back to the default. */
private class PrefsStore(private val p: SharedPreferences) : Store {
    override fun string(key: String, def: String) = try { p.getString(key, def) ?: def } catch (_: Exception) { def }
    override fun bool(key: String, def: Boolean) = try { p.getBoolean(key, def) } catch (_: Exception) { def }
    override fun float(key: String, def: Float) = try { p.getFloat(key, def) } catch (_: Exception) { def }
    override fun strings(key: String): Set<String> = try { p.getStringSet(key, null)?.toSet() ?: emptySet() } catch (_: Exception) { emptySet() }
    override fun save(values: Map<String, Any>) {
        val e = p.edit()
        for ((k, v) in values) {
            when (v) {
                is String -> e.putString(k, v)
                is Boolean -> e.putBoolean(k, v)
                is Float -> e.putFloat(k, v)
                is Set<*> -> e.putStringSet(k, v.map { it.toString() }.toSet())
            }
        }
        e.apply()
    }
}

private fun parseState(json: String): EngineState = try {
    val o = JSONObject(json)
    val peers = o.optJSONArray("peers")
    val streams = o.optJSONArray("streams")
    val ips = o.optJSONArray("ips")
    EngineState(
        ready = true,
        name = o.optString("name"),
        ips = (0 until (ips?.length() ?: 0)).map { ips!!.optString(it) },
        outputMs = o.optDouble("outputMs", 0.0),
        aaudio = o.optBoolean("aaudio"),
        captureMs = o.optDouble("captureMs", 0.0),
        receiving = o.optBoolean("receiving"),
        receiverError = o.optString("receiverError"),
        playLocal = o.optBoolean("playLocal", true),
        forwarded = o.optLong("forwarded"),
        sending = o.optBoolean("sending"),
        sendError = o.optString("sendError"),
        peers = (0 until (peers?.length() ?: 0)).map {
            val p = peers!!.getJSONObject(it)
            PeerInfo(p.optString("name"), p.optString("addr"), p.optBoolean("receiving"))
        },
        streams = (0 until (streams?.length() ?: 0)).map {
            val s = streams!!.getJSONObject(it)
            StreamInfo(
                s.optLong("id"), s.optString("name"), s.optString("from"), s.optDouble("bufferMs"), s.optDouble("captureMs", 0.0),
                s.optLong("lost"), s.optLong("late"), s.optLong("underruns"), s.optDouble("level").toFloat(), s.optInt("rate"),
            )
        },
    )
} catch (e: Exception) {
    EngineState(ready = true)
}

class MainActivity : ComponentActivity(), Host {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private lateinit var prefs: SharedPreferences
    private lateinit var model: UiModel

    /** Set once the user quits, so nothing starts the service again on the way out. */
    private var quitting = false
    private var updateFile: File? = null
    private val scopeBufs = HashMap<Int, FloatArray>()

    private val projectionLauncher = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        if (result.resultCode == Activity.RESULT_OK && result.data != null) {
            sendToService {
                it.putExtra(StreamService.EXTRA_RESULT_CODE, result.resultCode)
                it.putExtra(StreamService.EXTRA_RESULT_DATA, result.data)
            }
        } else {
            model.toast("ต้องกดอนุญาตก่อน จึงจะจับเสียงได้", error = true)
        }
    }

    private val micPermission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        if (granted) beginCapture() else model.toast("ต้องอนุญาตไมโครโฟน/บันทึกเสียงก่อน", error = true)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        prefs = getSharedPreferences(StreamService.PREFS, Context.MODE_PRIVATE)
        model = UiModel(PrefsStore(prefs))
        model.update = model.update.copy(
            version = Updater.current(this)?.let { if (it.isBeta) "$it (เบต้า)" else "$it" } ?: "รุ่นทดสอบ (dev)"
        )
        if (Build.VERSION.SDK_INT >= 33 &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1)
        }
        ensureService()
        setContent { AppUi(model, this@MainActivity) }
        scope.launch { pollEngine() }
        checkUpdate()
    }

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }

    private val handle: Long get() = StreamService.instance?.handle ?: 0L

    fun ensureService() {
        if (!quitting && StreamService.instance == null) {
            startForegroundService(Intent(this, StreamService::class.java))
        }
    }

    /** Refresh what the screens show a few times a second and keep the engine in step with the ticks. */
    private suspend fun pollEngine() {
        var applied = 0L
        var lastSend = ""
        var lastForward = ""
        var lastError = ""
        while (true) {
            val svc = StreamService.instance
            val h = svc?.handle ?: 0L
            if (h != 0L) {
                if (applied != h) {
                    // A new engine (first start or after a restart): give it the saved choices.
                    Native.setVolume(h, model.effectiveVolume)
                    Native.setMode(h, model.latencyMode)
                    lastSend = "-"
                    lastForward = "-"
                    applied = h
                }
                model.onEngineState(parseState(Native.stateJson(h)), System.currentTimeMillis())
                val send = model.sendTo.sorted().joinToString(",")
                if (send != lastSend) { Native.setSendDests(h, send); lastSend = send }
                val fw = model.forwardTo.sorted().joinToString(",")
                if (fw != lastForward) { Native.setForward(h, fw); lastForward = fw }
                if (svc!!.lastError.isNotEmpty() && svc.lastError != lastError) model.toast(svc.lastError, error = true)
                lastError = svc.lastError
            } else {
                model.engine = EngineState(ready = false)
                ensureService()
            }
            // Phones save Wi-Fi power only while the screen is off, so keep it on while streaming.
            val streaming = model.engine.sending || model.engine.streams.isNotEmpty()
            if (streaming && model.keepScreenOn) window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
            else window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
            delay(250)
        }
    }

    // ---- Host: what the screens ask for ---------------------------------------

    override fun startSending() {
        if (model.sendTo.isEmpty()) {
            model.toast("เลือกเครื่องปลายทางอย่างน้อย 1 เครื่องก่อน", error = true)
            return
        }
        if (model.source == "app" && appUid() < 0) {
            model.toast("เลือกแอปที่จะส่งเสียงก่อน", error = true)
            return
        }
        if (checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED) {
            beginCapture()
        } else {
            micPermission.launch(Manifest.permission.RECORD_AUDIO)
        }
    }

    private fun appUid(): Int = try {
        if (model.appPkg.isEmpty()) -1 else packageManager.getApplicationInfo(model.appPkg, 0).uid
    } catch (_: Exception) {
        -1
    }

    private fun beginCapture() {
        if (model.source == "mic") {
            sendToService {}
        } else {
            val mpm = getSystemService(Context.MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
            projectionLauncher.launch(mpm.createScreenCaptureIntent())
        }
    }

    private fun sendToService(extra: (Intent) -> Unit) {
        val i = Intent(this, StreamService::class.java)
            .setAction(StreamService.ACTION_SEND)
            .putExtra(StreamService.EXTRA_MODE, model.source)
            .putExtra(StreamService.EXTRA_UID, if (model.source == "app") appUid() else -1)
            .putExtra(StreamService.EXTRA_DESTS, model.sendTo.joinToString(","))
        extra(i)
        startForegroundService(i)
    }

    override fun stopSending() {
        startService(Intent(this, StreamService::class.java).setAction(StreamService.ACTION_STOP_SEND))
    }

    override fun setReceiving(on: Boolean) {
        val h = handle
        if (h == 0L) return
        if (on) {
            val err = Native.startReceiving(h)
            if (err.isNotEmpty()) model.toast("เปิดรับเสียงไม่ได้: $err", error = true)
        } else {
            Native.stopReceiving(h)
        }
    }

    override fun setPlayLocal(on: Boolean) {
        handle.takeIf { it != 0L }?.let { Native.setPlayLocal(it, on) }
    }

    override fun setVolume(volume: Float) {
        handle.takeIf { it != 0L }?.let { Native.setVolume(it, volume) }
    }

    override fun setMode(id: String) {
        handle.takeIf { it != 0L }?.let { Native.setMode(it, id) }
    }

    override fun rename(name: String) {
        handle.takeIf { it != 0L }?.let { Native.setName(it, name) }
    }

    override fun copy(text: String) {
        val cm = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        cm.setPrimaryClip(ClipData.newPlainText("IP", text))
    }

    override fun scope(send: Boolean, spectrum: Boolean, n: Int): FloatArray? {
        val h = handle
        if (h == 0L) return null
        val key = n * 4 + (if (send) 0 else 2) + (if (spectrum) 1 else 0)
        val buf = scopeBufs.getOrPut(key) { FloatArray(n) }
        return if (Native.scope(h, if (send) 0 else 1, if (spectrum) 1 else 0, buf)) buf else null
    }

    override fun apps(): List<AppInfo> {
        val pm = packageManager
        val intent = Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER)
        return pm.queryIntentActivities(intent, 0)
            .asSequence()
            .map { AppInfo(it.loadLabel(pm).toString(), it.activityInfo.packageName, it.activityInfo.applicationInfo.uid) }
            .filter { it.uid != applicationInfo.uid }
            .distinctBy { it.pkg }
            .sortedBy { it.label.lowercase() }
            .toList()
    }

    override fun checkUpdate() {
        if (model.update.busy) return
        val beta = model.betaUpdates
        if (model.update.readyBeta && !beta) {
            // Betas were just turned off: drop the beta that is waiting.
            updateFile = null
            model.update = model.update.copy(ready = null, readyBeta = false)
        }
        if (model.update.ready != null) return
        model.update = model.update.copy(busy = true, text = "กำลังตรวจสอบ…")
        scope.launch {
            try {
                when (val result = withContext(Dispatchers.IO) { Updater.check(this@MainActivity, beta) }) {
                    is Updater.Check.UpToDate -> model.update = model.update.copy(text = "เป็นเวอร์ชันล่าสุดแล้ว")
                    is Updater.Check.NoRelease -> model.update = model.update.copy(
                        text = if (beta) "ยังไม่พบเวอร์ชันที่เผยแพร่ให้ดาวน์โหลด" else "ยังไม่มีเวอร์ชันทางการ เปิดรับเวอร์ชันเบต้าเพื่อรับเวอร์ชันล่าสุด"
                    )
                    is Updater.Check.Newer -> {
                        val rel = result.release
                        val file = withContext(Dispatchers.IO) {
                            Updater.download(this@MainActivity, rel) { p ->
                                runOnUiThread { model.update = model.update.copy(text = "กำลังดาวน์โหลด v${rel.version} ($p%)") }
                            }
                        }
                        updateFile = file
                        model.update = model.update.copy(
                            text = "v${rel.version} พร้อมติดตั้ง",
                            ready = rel.version.toString(),
                            readyBeta = rel.version.isBeta,
                        )
                    }
                }
            } catch (e: Exception) {
                val msg = e.message ?: e.javaClass.simpleName
                val offline = msg.contains("Unable to resolve", true) || msg.contains("timeout", true) || msg.contains("failed to connect", true)
                model.update = model.update.copy(
                    text = if (offline) "ติดต่อ GitHub ไม่ได้ ตรวจอินเทอร์เน็ตแล้วลองใหม่" else "ตรวจสอบอัปเดตไม่สำเร็จ: $msg"
                )
            } finally {
                model.update = model.update.copy(busy = false)
            }
        }
    }

    override fun installUpdate() {
        val f = updateFile ?: return
        if (!Updater.install(this, f)) {
            model.toast("อนุญาต \"ติดตั้งแอปที่ไม่รู้จัก\" ให้ Stream Sound แล้วกดอัปเดตอีกครั้ง", error = true)
        }
    }

    /** Stop streaming, close the screen and end the process (the service does the last step). */
    override fun quit() {
        quitting = true
        if (StreamService.instance != null) {
            startService(Intent(this, StreamService::class.java).setAction(StreamService.ACTION_QUIT))
            finishAndRemoveTask()
        } else {
            finishAndRemoveTask()
            Process.killProcess(Process.myPid())
        }
    }
}
