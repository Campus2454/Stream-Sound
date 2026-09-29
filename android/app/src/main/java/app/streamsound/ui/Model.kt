package app.streamsound.ui

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

// Plain Compose only in this package (no android.*), so the screens can also
// be rendered on a desktop JVM for previews.

const val AUDIO_PORT = 47800

data class PeerInfo(val name: String, val addr: String, val receiving: Boolean, val manual: Boolean = false)

data class StreamInfo(
    val id: Long,
    val name: String,
    val from: String,
    val bufferMs: Double,
    val captureMs: Double,
    val lost: Long,
    val late: Long,
    val underruns: Long,
    val level: Float,
    val rate: Int,
)

/** What the engine reports, refreshed a few times a second. */
data class EngineState(
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
    val sendError: String = "",
    val peers: List<PeerInfo> = emptyList(),
    val streams: List<StreamInfo> = emptyList(),
)

data class AppInfo(val label: String, val pkg: String, val uid: Int)

data class UpdateInfo(
    /** This install's version as shown, e.g. "0.2" or "0.2.3 (เบต้า)". */
    val version: String = "",
    val text: String = "",
    val busy: Boolean = false,
    /** Version of a downloaded update waiting to be installed, e.g. "0.2.3". */
    val ready: String? = null,
    val readyBeta: Boolean = false,
)

data class Toast(val text: String, val error: Boolean, val at: Long)

enum class Tab { Send, Receive, Settings }

enum class VizStyle { Bars, Wave }

/** Saved settings (SharedPreferences on the phone, a map in previews). */
interface Store {
    fun string(key: String, def: String): String
    fun bool(key: String, def: Boolean): Boolean
    fun float(key: String, def: Float): Float
    fun strings(key: String): Set<String>
    fun save(values: Map<String, Any>)
}

/** Everything the screens need from the phone. */
interface Host {
    fun startSending()
    fun stopSending()
    fun setReceiving(on: Boolean)
    fun setPlayLocal(on: Boolean)
    fun setVolume(volume: Float)
    fun setMode(id: String)
    fun rename(name: String)
    fun copy(text: String)
    fun checkUpdate()
    fun installUpdate()
    fun quit()
    /** Apps that can be captured on their own. Blocking. */
    fun apps(): List<AppInfo>
    /** Live audio for the visualizer: waveform or spectrum, 0..1 per column; null when that side is idle. */
    fun scope(send: Boolean, spectrum: Boolean, n: Int): FloatArray?
}

/** "192.168.1.20" → "192.168.1.20:47800"; null if it isn't an IPv4 address. */
fun normalizeAddr(text: String): String? {
    val t = text.trim()
    val m = Regex("""^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})(?::(\d{1,5}))?$""").matchEntire(t) ?: return null
    val parts = (1..4).map { m.groupValues[it].toInt() }
    if (parts.any { it > 255 }) return null
    val port = m.groupValues[5].takeIf { it.isNotEmpty() }?.toInt() ?: AUDIO_PORT
    if (port !in 1..65535) return null
    return parts.joinToString(".") + ":" + port
}

class UiModel(private val store: Store) {
    var tab by mutableStateOf(Tab.Send)
    var engine by mutableStateOf(EngineState())

    /** "system", "app" or "mic". */
    var source by mutableStateOf(store.string("mode", "system"))
    var appPkg by mutableStateOf(store.string("appPkg", ""))
    var appLabel by mutableStateOf(store.string("appLabel", ""))

    /** Ticked devices as "ip:port". */
    val sendTo = mutableStateListOf<String>().apply { addAll(store.strings("sendTo")) }
    val forwardTo = mutableStateListOf<String>().apply { addAll(store.strings("forwardTo")) }
    /** Typed-in addresses, normalised to "ip:port". */
    val manual = mutableStateListOf<String>().apply { addAll(store.strings("manual").mapNotNull { normalizeAddr(it) }.distinct().sorted()) }

    var volume by mutableFloatStateOf(store.float("volume", 1f).coerceIn(0f, 1.5f))
    var muted by mutableStateOf(store.bool("muted", false))
    var latencyMode by mutableStateOf(store.string("latencyMode", "balanced"))
    var visual by mutableStateOf(if (store.string("visual", "bars") == "wave") VizStyle.Wave else VizStyle.Bars)
    var keepScreenOn by mutableStateOf(store.bool("keepScreenOn", true))
    var autoReceive by mutableStateOf(store.bool("autoReceive", true))
    /** Also update to beta versions (vX.Y.Z), not only official ones. */
    var betaUpdates by mutableStateOf(store.bool("betaUpdates", true))
    var nameInput by mutableStateOf(store.string("name", ""))

    var relayOpen by mutableStateOf(false)
    var expanded by mutableStateOf<Long?>(null)
    var toast by mutableStateOf<Toast?>(null)
    var update by mutableStateOf(UpdateInfo())

    /** Per stream: underrun count last seen and when it last went up (ms). */
    private val stutter = HashMap<Long, Pair<Long, Long>>()

    fun save() {
        store.save(
            mapOf(
                "mode" to source,
                "appPkg" to appPkg,
                "appLabel" to appLabel,
                "sendTo" to sendTo.toSet(),
                "forwardTo" to forwardTo.toSet(),
                "manual" to manual.toSet(),
                "volume" to volume,
                "muted" to muted,
                "latencyMode" to latencyMode,
                "visual" to if (visual == VizStyle.Wave) "wave" else "bars",
                "keepScreenOn" to keepScreenOn,
                "autoReceive" to autoReceive,
                "betaUpdates" to betaUpdates,
            )
        )
    }

    fun saveName(name: String) = store.save(mapOf("name" to name))

    fun toast(text: String, error: Boolean = false) {
        toast = Toast(text, error, System.nanoTime())
    }

    fun toggleVisual() {
        visual = if (visual == VizStyle.Bars) VizStyle.Wave else VizStyle.Bars
        save()
    }

    /** Engine volume: 0 while muted. */
    val effectiveVolume: Float get() = if (muted) 0f else volume

    /** Discovered devices plus typed-in addresses, without duplicates. */
    fun targets(): List<PeerInfo> {
        val v = engine.peers.toMutableList()
        for (a in manual) {
            if (v.none { it.addr == a }) v.add(PeerInfo(a.substringBefore(':'), a, receiving = true, manual = true))
        }
        return v
    }

    fun toggle(list: MutableList<String>, key: String) {
        if (!list.remove(key)) list.add(key)
        save()
    }

    fun onEngineState(s: EngineState, nowMs: Long) {
        for (st in s.streams) {
            val e = stutter[st.id]
            if (e == null) stutter[st.id] = st.underruns to 0L
            else if (st.underruns > e.first) stutter[st.id] = st.underruns to nowMs
        }
        stutter.keys.retainAll(s.streams.map { it.id }.toSet())
        engine = s
    }

    /** Stuttered in the last 10 seconds. */
    fun recentStutter(id: Long, nowMs: Long): Boolean {
        val at = stutter[id]?.second ?: return false
        return at != 0L && nowMs - at < 10_000
    }
}
