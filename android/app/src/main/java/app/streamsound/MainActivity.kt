package app.streamsound

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.media.projection.MediaProjectionManager
import android.os.Build
import android.os.Bundle
import android.os.Process
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject

private val Red = Color(0xFFE51A2E)
private val RedDim = Color(0xFF7A101A)
private val Bg = Color(0xFF0C0C0D)
private val Panel = Color(0xFF161618)
private val Muted = Color(0xFF9A9AA0)
private val ErrorText = Color(0xFFFF8080)

data class PeerInfo(val id: String, val name: String, val addr: String, val receiving: Boolean)
data class StreamInfo(
    val name: String, val bufferMs: Double, val targetMs: Double, val captureMs: Double,
    val lost: Long, val late: Long, val underruns: Long, val level: Float, val rate: Int,
)
data class UiState(
    val ready: Boolean = false,
    val name: String = "",
    val ips: List<String> = emptyList(),
    val outputMs: Double = 0.0,
    val aaudio: Boolean = false,
    val captureMs: Double = 0.0,
    val receiving: Boolean = false,
    val receiverError: String = "",
    val playLocal: Boolean = true,
    val forwarded: Long = 0,
    val sending: Boolean = false,
    val sentPackets: Long = 0,
    val sendLevel: Float = 0f,
    val sendError: String = "",
    val peers: List<PeerInfo> = emptyList(),
    val streams: List<StreamInfo> = emptyList(),
)
data class AppInfo(val label: String, val uid: Int)

private fun parseState(json: String, ready: Boolean): UiState = try {
    val o = JSONObject(json)
    val peers = o.optJSONArray("peers")
    val streams = o.optJSONArray("streams")
    val ips = o.optJSONArray("ips")
    UiState(
        ready = ready,
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
        sentPackets = o.optLong("sentPackets"),
        sendLevel = o.optDouble("sendLevel", 0.0).toFloat(),
        sendError = o.optString("sendError"),
        peers = (0 until (peers?.length() ?: 0)).map {
            val p = peers!!.getJSONObject(it)
            PeerInfo(p.optString("id"), p.optString("name"), p.optString("addr"), p.optBoolean("receiving"))
        },
        streams = (0 until (streams?.length() ?: 0)).map {
            val s = streams!!.getJSONObject(it)
            StreamInfo(
                s.optString("name"), s.optDouble("bufferMs"), s.optDouble("targetMs"), s.optDouble("captureMs", 0.0),
                s.optLong("lost"), s.optLong("late"), s.optLong("underruns"), s.optDouble("level").toFloat(), s.optInt("rate"),
            )
        },
    )
} catch (e: Exception) {
    UiState(ready = ready)
}

private val MODES = listOf(
    Triple("game", "เกม (หน่วงต่ำสุด)", "เสียงตรงกับภาพที่สุด เสียงที่มาช้าจะถูกทิ้ง ถ้า Wi-Fi สะดุดอาจได้ยินเสียงแตกสั้น ๆ"),
    Triple("balanced", "สมดุล", "หน่วงต่ำและไม่สะดุด ใช้ได้ทั่วไป (แนะนำ)"),
    Triple("music", "ฟังเพลง (เสถียรสุด)", "หน่วงมากขึ้น แต่ทน Wi-Fi ที่ไม่เสถียรได้ดีที่สุด"),
)

/** "หน่วงรวม ~35 ms (จับเสียง 10 + บัฟเฟอร์ 15 + ลำโพง 10)" */
private fun delayLine(s: StreamInfo, outMs: Double): String {
    val parts = mutableListOf<String>()
    if (s.captureMs > 0) parts.add("จับเสียง ${s.captureMs.toInt()}")
    parts.add("บัฟเฟอร์ ${s.bufferMs.toInt()}")
    if (outMs > 0) parts.add("ลำโพง ${outMs.toInt()}")
    return "หน่วงรวม ~${(s.captureMs + s.bufferMs + outMs).toInt()} ms (${parts.joinToString(" + ")})"
}

class MainActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        if (Build.VERSION.SDK_INT >= 33 &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1)
        }
        ensureService()
        setContent {
            MaterialTheme(
                colorScheme = darkColorScheme(
                    primary = Red,
                    onPrimary = Color.White,
                    secondary = Red,
                    background = Bg,
                    surface = Panel,
                    onSurface = Color.White,
                    onBackground = Color.White,
                )
            ) {
                AppScreen(this)
            }
        }
    }

    /** Set once the user quits, so nothing starts the service again on the way out. */
    private var quitting = false

    fun ensureService() {
        if (!quitting && StreamService.instance == null) {
            startForegroundService(Intent(this, StreamService::class.java))
        }
    }

    /** Stop streaming, close the screen and end the process (the service does the last step). */
    fun quit() {
        quitting = true
        if (StreamService.instance != null) {
            startService(Intent(this, StreamService::class.java).setAction(StreamService.ACTION_QUIT))
            finishAndRemoveTask()
        } else {
            finishAndRemoveTask()
            Process.killProcess(Process.myPid())
        }
    }

    fun launcherApps(): List<AppInfo> {
        val pm = packageManager
        val intent = Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER)
        return pm.queryIntentActivities(intent, 0)
            .asSequence()
            .map { AppInfo(it.loadLabel(pm).toString(), it.activityInfo.applicationInfo.uid) }
            .filter { it.uid != applicationInfo.uid }
            .distinctBy { it.uid }
            .sortedBy { it.label.lowercase() }
            .toList()
    }
}

@Composable
private fun AppScreen(activity: MainActivity) {
    val prefs = remember { activity.getSharedPreferences("ui", Context.MODE_PRIVATE) }
    var state by remember { mutableStateOf(UiState()) }
    var sourceMode by remember { mutableStateOf(prefs.getString("mode", "system") ?: "system") }
    var apps by remember { mutableStateOf<List<AppInfo>>(emptyList()) }
    var selectedApp by remember { mutableStateOf<AppInfo?>(null) }
    var showApps by remember { mutableStateOf(false) }
    val sendTo = remember { mutableStateListOf<String>() }
    val forwardTo = remember { mutableStateListOf<String>() }
    val manual = remember { mutableStateListOf<String>().apply { addAll(prefs.getStringSet("manual", emptySet())!!.sorted()) } }
    var manualInput by remember { mutableStateOf("") }
    var volume by remember { mutableFloatStateOf(prefs.getFloat("volume", 1f)) }
    var latencyMode by remember { mutableStateOf(prefs.getString(StreamService.PREF_LATENCY_MODE, "balanced") ?: "balanced") }
    var message by remember { mutableStateOf("") }
    var pendingDests by remember { mutableStateOf("") }

    // ---- self-update from GitHub Releases ----
    val scope = rememberCoroutineScope()
    val installedBuild = remember { Updater.currentBuild(activity) }
    var updateRelease by remember { mutableStateOf<Updater.Release?>(null) }
    var updateFile by remember { mutableStateOf<java.io.File?>(null) }
    var updateText by remember { mutableStateOf("") }
    var updateBusy by remember { mutableStateOf(false) }
    fun runUpdateCheck() {
        if (updateBusy) return
        updateBusy = true
        scope.launch {
            updateText = "กำลังตรวจสอบอัปเดต…"
            try {
                val result = withContext(Dispatchers.IO) { Updater.check(activity) }
                when (result) {
                    is Updater.Check.UpToDate -> updateText = "เป็นเวอร์ชันล่าสุดแล้ว"
                    is Updater.Check.NoRelease -> updateText = "ยังไม่มีเวอร์ชันที่เผยแพร่บน GitHub"
                    is Updater.Check.Newer -> {
                        val rel = result.release
                        updateRelease = rel
                        val file = withContext(Dispatchers.IO) {
                            Updater.download(activity, rel) { p ->
                                activity.runOnUiThread { updateText = "กำลังดาวน์โหลด build ${rel.build} ($p%)" }
                            }
                        }
                        updateFile = file
                        updateText = "อัปเดตพร้อมติดตั้ง"
                    }
                }
            } catch (e: Exception) {
                updateText = "ตรวจสอบอัปเดตไม่ได้: ${e.message ?: e.javaClass.simpleName}"
            } finally {
                updateBusy = false
            }
        }
    }
    LaunchedEffect(Unit) { runUpdateCheck() }

    // Poll the engine a few times a second.
    LaunchedEffect(Unit) {
        var applied = false
        while (true) {
            val svc = StreamService.instance
            if (svc != null && svc.handle != 0L) {
                if (!applied) {
                    Native.setVolume(svc.handle, volume)
                    Native.setMode(svc.handle, latencyMode)
                    applied = true
                }
                state = parseState(Native.stateJson(svc.handle), true)
                if (svc.lastError.isNotEmpty() && message.isEmpty()) message = svc.lastError
            } else {
                state = UiState(ready = false)
                activity.ensureService()
            }
            delay(250)
        }
    }

    // Phones cut Wi-Fi power saving only while the screen is on, so keep it on
    // while audio is flowing and this app is open.
    val streaming = state.sending || state.streams.isNotEmpty()
    LaunchedEffect(streaming) {
        if (streaming) activity.window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        else activity.window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
    }

    // Every destination the user can tick: discovered devices plus typed IPs.
    val targets: List<PeerInfo> = state.peers + manual.map { PeerInfo("ip:$it", it, if (it.contains(':')) it else "$it:47800", true) }
    fun addrsOf(ids: List<String>) = targets.filter { ids.contains(it.id) }.joinToString(",") { it.addr }

    // Keep the engine's destination lists in sync with the ticks.
    val sendAddrs = addrsOf(sendTo)
    val forwardAddrs = addrsOf(forwardTo)
    LaunchedEffect(sendAddrs, state.ready) {
        StreamService.instance?.handle?.takeIf { it != 0L }?.let { Native.setSendDests(it, sendAddrs) }
    }
    LaunchedEffect(forwardAddrs, state.ready) {
        StreamService.instance?.handle?.takeIf { it != 0L }?.let { Native.setForward(it, forwardAddrs) }
    }

    fun sendToService(mode: String, extra: (Intent) -> Unit) {
        val i = Intent(activity, StreamService::class.java)
            .setAction(StreamService.ACTION_SEND)
            .putExtra(StreamService.EXTRA_MODE, mode)
            .putExtra(StreamService.EXTRA_UID, selectedApp?.uid ?: -1)
            .putExtra(StreamService.EXTRA_DESTS, pendingDests)
        extra(i)
        activity.startForegroundService(i)
    }

    val projectionLauncher = rememberLauncherForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        if (result.resultCode == Activity.RESULT_OK && result.data != null) {
            sendToService(sourceMode) {
                it.putExtra(StreamService.EXTRA_RESULT_CODE, result.resultCode)
                it.putExtra(StreamService.EXTRA_RESULT_DATA, result.data)
            }
        } else {
            message = "ต้องกดอนุญาตก่อน จึงจะจับเสียงได้"
        }
    }

    fun beginCapture() {
        if (sourceMode == "mic") {
            sendToService("mic") {}
        } else {
            val mpm = activity.getSystemService(Context.MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
            projectionLauncher.launch(mpm.createScreenCaptureIntent())
        }
    }

    val micPermission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        if (granted) beginCapture() else message = "ต้องอนุญาตไมโครโฟน/บันทึกเสียงก่อน"
    }

    fun onStartSending() {
        message = ""
        if (sendAddrs.isEmpty()) {
            message = "เลือกเครื่องปลายทางอย่างน้อย 1 เครื่องก่อน"
            return
        }
        if (sourceMode == "app" && selectedApp == null) {
            message = "เลือกแอปที่จะส่งเสียงก่อน"
            return
        }
        pendingDests = sendAddrs
        prefs.edit().putString("mode", sourceMode).apply()
        if (activity.checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED) {
            beginCapture()
        } else {
            micPermission.launch(Manifest.permission.RECORD_AUDIO)
        }
    }

    Column(
        Modifier
            .fillMaxSize()
            .background(Bg)
            .verticalScroll(rememberScrollState())
            .padding(16.dp)
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("Stream Sound", color = Red, fontSize = 26.sp, fontWeight = FontWeight.Bold)
            Spacer(Modifier.weight(1f))
            Text(
                if (!state.ready) "กำลังเริ่ม…"
                else "เครื่องนี้: ${state.name}" + (state.ips.firstOrNull()?.let { " ($it)" } ?: " (ไม่ได้ต่อ Wi-Fi)"),
                color = Muted, fontSize = 13.sp,
            )
        }
        if (message.isNotEmpty()) {
            Text(message, color = ErrorText, modifier = Modifier.padding(top = 6.dp).clickable { message = "" })
        }
        Spacer(Modifier.height(10.dp))

        val readyFile = updateFile
        val readyRelease = updateRelease
        if (readyFile != null && readyRelease != null) {
            Card(
                colors = CardDefaults.cardColors(containerColor = RedDim),
                shape = RoundedCornerShape(12.dp),
                modifier = Modifier.fillMaxWidth().padding(bottom = 12.dp),
            ) {
                Column(Modifier.padding(14.dp)) {
                    Text("มีเวอร์ชันใหม่พร้อมแล้ว (build ${readyRelease.build})", color = Color.White, fontWeight = FontWeight.Bold)
                    Spacer(Modifier.height(6.dp))
                    Button(
                        onClick = {
                            if (!Updater.install(activity, readyFile)) {
                                message = "อนุญาต \"ติดตั้งแอปที่ไม่รู้จัก\" ให้ Stream Sound แล้วกดติดตั้งอีกครั้ง"
                            }
                        },
                        colors = ButtonDefaults.buttonColors(containerColor = Color.White, contentColor = RedDim),
                    ) { Text("ติดตั้งอัปเดต") }
                }
            }
        }

        // ---- latency mode ----
        Section("โหมด") {
            MODES.forEach { (id, label, _) ->
                SourceOption(label, latencyMode == id, true) {
                    latencyMode = id
                    prefs.edit().putString(StreamService.PREF_LATENCY_MODE, id).apply()
                    StreamService.instance?.handle?.takeIf { h -> h != 0L }?.let { h -> Native.setMode(h, id) }
                }
            }
            Text(MODES.firstOrNull { it.first == latencyMode }?.third ?: "", color = Muted, fontSize = 12.sp)
            Text(
                "คุณภาพเสียงเท่ากันทุกโหมด (ไม่บีบอัด) ต่างกันแค่ความหน่วงกับความทนต่อ Wi-Fi สะดุด " +
                    "มือถือหน่วงน้อยสุดเมื่อเปิดแอปนี้ค้างไว้บนจอ",
                color = Muted, fontSize = 12.sp,
            )
        }

        // ---- send ----
        Section("ส่งเสียง") {
            Text("แหล่งเสียง", color = Muted)
            SourceOption("เสียงทั้งเครื่อง", sourceMode == "system", !state.sending) { sourceMode = "system" }
            SourceOption("เฉพาะแอป", sourceMode == "app", !state.sending) {
                sourceMode = "app"
                if (apps.isEmpty()) apps = activity.launcherApps()
            }
            if (sourceMode == "app") {
                OutlinedButton(
                    onClick = { if (apps.isEmpty()) apps = activity.launcherApps(); showApps = !showApps },
                    enabled = !state.sending,
                    modifier = Modifier.fillMaxWidth().padding(start = 32.dp),
                ) { Text(selectedApp?.label ?: "เลือกแอป…") }
                if (showApps && !state.sending) {
                    Column(Modifier.padding(start = 32.dp)) {
                        apps.forEach { app ->
                            TextButton(onClick = { selectedApp = app; showApps = false }) {
                                Text(app.label, color = if (app == selectedApp) Red else Color.White)
                            }
                        }
                    }
                }
            }
            SourceOption("ไมโครโฟน", sourceMode == "mic", !state.sending) { sourceMode = "mic" }
            if (sourceMode != "mic") {
                Text(
                    "Android จะถามอนุญาต \"บันทึกหน้าจอ\" (ใช้แค่เสียง) และบางแอปไม่ยอมให้จับเสียง",
                    color = Muted, fontSize = 12.sp,
                )
            }
            Spacer(Modifier.height(8.dp))
            Text("ส่งไปที่ (เลือกได้หลายเครื่อง)", fontWeight = FontWeight.Bold)
            Targets(targets, sendTo)
            Spacer(Modifier.height(8.dp))
            if (!state.sending) {
                BigButton("เริ่มส่งเสียง", Red) { onStartSending() }
            } else {
                Text("● กำลังส่ง  ${state.sentPackets} แพ็กเก็ต", color = Red)
                if (state.captureMs > 0) Text("หน่วงตอนจับเสียง ~${state.captureMs.toInt()} ms", color = Muted, fontSize = 12.sp)
                Level(state.sendLevel)
                if (state.sendError.isNotEmpty()) Text("ปัญหา: ${state.sendError}", color = ErrorText)
                BigButton("หยุดส่ง", RedDim) {
                    activity.startService(Intent(activity, StreamService::class.java).setAction(StreamService.ACTION_STOP_SEND))
                }
            }
        }

        // ---- receive ----
        Section("รับเสียง") {
            SwitchRow("เปิดรับเสียงจากเครื่องอื่น", state.receiving) { on ->
                StreamService.instance?.handle?.takeIf { it != 0L }?.let {
                    if (on) {
                        val err = Native.startReceiving(it)
                        if (err.isNotEmpty()) message = "เปิดรับเสียงไม่ได้: $err"
                    } else Native.stopReceiving(it)
                }
            }
            SwitchRow("เล่นเสียงที่เครื่องนี้", state.playLocal) { on ->
                StreamService.instance?.handle?.takeIf { it != 0L }?.let { Native.setPlayLocal(it, on) }
            }
            Text("ระดับเสียง", color = Muted)
            Slider(value = volume, onValueChange = {
                volume = it
                StreamService.instance?.handle?.takeIf { h -> h != 0L }?.let { h -> Native.setVolume(h, it) }
            }, onValueChangeFinished = { prefs.edit().putFloat("volume", volume).apply() }, valueRange = 0f..1.5f)
            if (state.receiverError.isNotEmpty()) Text("ลำโพง: ${state.receiverError}", color = ErrorText)
            if (state.streams.isEmpty() && state.receiving) Text("รอเสียงจากเครื่องอื่น…", color = Muted)
            state.streams.forEach { s ->
                Spacer(Modifier.height(6.dp))
                Text(s.name, fontWeight = FontWeight.Bold)
                Text(delayLine(s, state.outputMs), fontSize = 13.sp)
                Text(
                    "หาย ${s.lost} · มาช้า ${s.late} · สะดุด ${s.underruns} · ${s.rate / 1000f} kHz",
                    color = Muted, fontSize = 12.sp,
                )
                Level(s.level)
            }
            if (state.streams.isNotEmpty()) {
                Text(
                    "ไม่รวมเวลาเดินทางใน Wi-Fi (~2–10 ms) · ลำโพง: " + if (state.aaudio) "AAudio" else "AudioTrack",
                    color = Muted, fontSize = 12.sp,
                )
            }
            Spacer(Modifier.height(8.dp))
            Text("ส่งต่อเสียงที่รับได้ไปเครื่องอื่น (ต่อเป็นทอด)", fontWeight = FontWeight.Bold)
            Targets(targets, forwardTo)
            if (state.forwarded > 0) Text("ส่งต่อแล้ว ${state.forwarded} แพ็กเก็ต", color = Muted, fontSize = 12.sp)
        }

        // ---- devices ----
        Section("อุปกรณ์") {
            Text("เครื่องที่เปิดแอปนี้ใน Wi-Fi เดียวกันจะขึ้นเองอัตโนมัติ", color = Muted, fontSize = 12.sp)
            Row(verticalAlignment = Alignment.CenterVertically) {
                OutlinedTextField(
                    value = manualInput, onValueChange = { manualInput = it },
                    placeholder = { Text("IP เช่น 192.168.1.20") }, singleLine = true,
                    modifier = Modifier.weight(1f),
                )
                Spacer(Modifier.width(8.dp))
                Button(onClick = {
                    val ip = manualInput.trim()
                    if (Regex("""^\d{1,3}(\.\d{1,3}){3}(:\d{1,5})?$""").matches(ip)) {
                        if (!manual.contains(ip)) manual.add(ip)
                        prefs.edit().putStringSet("manual", manual.toSet()).apply()
                        manualInput = ""
                    } else message = "IP ไม่ถูกต้อง"
                }) { Text("เพิ่ม") }
            }
            manual.toList().forEach { ip ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(ip, modifier = Modifier.weight(1f))
                    TextButton(onClick = {
                        manual.remove(ip); sendTo.remove("ip:$ip"); forwardTo.remove("ip:$ip")
                        prefs.edit().putStringSet("manual", manual.toSet()).apply()
                    }) { Text("ลบ", color = Red) }
                }
            }
        }

        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
            Text("เวอร์ชัน build $installedBuild", color = Muted, fontSize = 12.sp)
            Spacer(Modifier.width(8.dp))
            Text(updateText, color = Muted, fontSize = 12.sp, modifier = Modifier.weight(1f))
            TextButton(onClick = { runUpdateCheck() }, enabled = !updateBusy) { Text("ตรวจสอบอัปเดต", color = Red) }
        }

        OutlinedButton(
            onClick = { activity.quit() },
            modifier = Modifier.fillMaxWidth(),
        ) { Text("ปิดแอปและหยุดทั้งหมด", color = Muted) }
        Spacer(Modifier.height(24.dp))
    }
}

@Composable
private fun Section(title: String, content: @Composable () -> Unit) {
    Card(
        colors = CardDefaults.cardColors(containerColor = Panel),
        shape = RoundedCornerShape(12.dp),
        modifier = Modifier.fillMaxWidth().padding(bottom = 12.dp),
    ) {
        Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(title, fontSize = 20.sp, fontWeight = FontWeight.Bold, color = Color.White)
            content()
        }
    }
}

@Composable
private fun SourceOption(label: String, selected: Boolean, enabled: Boolean, onClick: () -> Unit) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth().clickable(enabled = enabled) { onClick() },
    ) {
        RadioButton(selected = selected, onClick = onClick, enabled = enabled)
        Text(label)
    }
}

@Composable
private fun Targets(targets: List<PeerInfo>, selected: MutableList<String>) {
    if (targets.isEmpty()) {
        Text("ยังไม่พบเครื่องอื่น เปิดแอปนี้บนอีกเครื่อง หรือเพิ่ม IP เอง", color = Muted, fontSize = 13.sp)
    }
    targets.forEach { t ->
        val on = selected.contains(t.id)
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.fillMaxWidth().clickable { if (on) selected.remove(t.id) else selected.add(t.id) },
        ) {
            Checkbox(checked = on, onCheckedChange = { if (it) selected.add(t.id) else selected.remove(t.id) })
            Column {
                Text(t.name)
                Text(
                    t.addr.substringBefore(':') + if (t.receiving) "" else "  (ไม่ได้เปิดรับ)",
                    color = Muted, fontSize = 12.sp,
                )
            }
        }
    }
}

@Composable
private fun SwitchRow(label: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
        Text(label, modifier = Modifier.weight(1f))
        Switch(checked = checked, onCheckedChange = onChange)
    }
}

@Composable
private fun Level(level: Float) {
    LinearProgressIndicator(
        progress = { level.coerceIn(0f, 1f) },
        color = Red,
        trackColor = Color(0xFF2A2A2E),
        modifier = Modifier.fillMaxWidth().height(6.dp).padding(vertical = 1.dp),
    )
}

@Composable
private fun BigButton(label: String, color: Color, onClick: () -> Unit) {
    Button(
        onClick = onClick,
        colors = ButtonDefaults.buttonColors(containerColor = color, contentColor = Color.White),
        modifier = Modifier.fillMaxWidth().height(48.dp),
    ) { Text(label, fontSize = 18.sp) }
}
