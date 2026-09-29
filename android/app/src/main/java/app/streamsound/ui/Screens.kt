package app.streamsound.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import kotlin.math.roundToInt

/** The whole app: header, the tab's page, bottom navigation, toasts. */
@Composable
fun AppUi(m: UiModel, host: Host) {
    Column(Modifier.fillMaxSize().background(C.Bg)) {
        Header(m, host)
        Box(Modifier.weight(1f).fillMaxWidth()) {
            key(m.tab) {
                Column(
                    Modifier
                        .fillMaxSize()
                        .verticalScroll(rememberScrollState())
                        .padding(horizontal = 16.dp)
                ) {
                    Gap(4.dp)
                    UpdateBanner(m, host)
                    when (m.tab) {
                        Tab.Send -> SendTab(m, host)
                        Tab.Receive -> ReceiveTab(m, host)
                        Tab.Settings -> SettingsTab(m, host)
                    }
                    Gap(16.dp)
                }
            }
            ToastHost(m, Modifier.align(Alignment.BottomCenter))
        }
        BottomNav(m)
    }
}

@Composable
private fun Header(m: UiModel, host: Host) {
    val e = m.engine
    Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 14.dp, bottom = 10.dp), verticalAlignment = Alignment.CenterVertically) {
        Logo(30.dp)
        Spacer(Modifier.width(10.dp))
        Text("Stream Sound", color = C.Text, fontSize = 19.sp, fontWeight = FontWeight.SemiBold)
        Spacer(Modifier.weight(1f).widthIn(min = 8.dp))
        val ip = e.ips.firstOrNull()
        val (text, dot) = when {
            !e.ready -> "กำลังเริ่ม…" to C.Text3
            ip == null -> "${e.name} · ไม่ได้ต่อ Wi-Fi" to C.Amber
            else -> "${e.name} · $ip" to C.Green
        }
        Row(
            Modifier
                .clip(CircleShape)
                .background(C.Surface2)
                .clickable(enabled = ip != null) {
                    host.copy(ip ?: "")
                    m.toast("คัดลอก $ip แล้ว")
                }
                .padding(horizontal = 12.dp, vertical = 7.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(Modifier.size(7.dp).clip(CircleShape).background(dot))
            Spacer(Modifier.width(7.dp))
            Text(text, color = C.Text2, fontSize = 12.5.sp, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.widthIn(max = 190.dp))
        }
    }
}

@Composable
private fun BottomNav(m: UiModel) {
    val items = listOf(Triple(Tab.Send, Glyph.Send, "ส่งเสียง"), Triple(Tab.Receive, Glyph.Receive, "รับเสียง"), Triple(Tab.Settings, Glyph.Settings, "ตั้งค่า"))
    Column(Modifier.fillMaxWidth().background(C.Surface)) {
        Box(Modifier.fillMaxWidth().height(1.dp).background(C.Outline))
        Row(Modifier.fillMaxWidth().height(68.dp)) {
            items.forEach { (tab, glyph, label) ->
                val on = m.tab == tab
                val k by animateFloatAsState(if (on) 1f else 0f, tween(160), label = "nav")
                val dot = when {
                    tab == Tab.Send && m.engine.sending -> C.RedHi
                    tab == Tab.Receive && m.engine.streams.isNotEmpty() -> C.Green
                    else -> null
                }
                Column(
                    Modifier
                        .weight(1f)
                        .fillMaxSize()
                        .clickable { m.tab = tab },
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.Center,
                ) {
                    Box {
                        Box(
                            Modifier
                                .size(58.dp, 30.dp)
                                .clip(CircleShape)
                                .background(C.Red.copy(alpha = k)),
                            contentAlignment = Alignment.Center,
                        ) {
                            GlyphIcon(glyph, lerp(C.Text2, Color.White, k), 21.dp)
                        }
                        if (dot != null) {
                            Box(
                                Modifier
                                    .align(Alignment.TopEnd)
                                    .padding(top = 3.dp, end = 10.dp)
                                    .size(8.dp)
                                    .clip(CircleShape)
                                    .background(if (on) Color.White else dot)
                            )
                        }
                    }
                    Gap(4.dp)
                    Text(label, color = lerp(C.Text2, C.Text, k), fontSize = 12.5.sp)
                }
            }
        }
    }
}

@Composable
private fun UpdateBanner(m: UiModel, host: Host) {
    val version = m.update.ready ?: return
    val shape = RoundedCornerShape(16.dp)
    Row(
        Modifier
            .fillMaxWidth()
            .padding(bottom = 12.dp)
            .clip(shape)
            .background(Brush.horizontalGradient(listOf(C.RedDeep, C.Red)))
            .padding(14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                if (m.update.readyBeta) "มีเวอร์ชันเบต้าใหม่ v$version" else "มีเวอร์ชันใหม่ v$version",
                color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.SemiBold,
            )
            Text("ติดตั้งทับได้เลย การตั้งค่ายังอยู่ครบ", color = Color.White.copy(alpha = 0.75f), fontSize = 12.5.sp)
        }
        Box(
            Modifier.clip(RoundedCornerShape(10.dp)).background(Color.White).clickable { host.installUpdate() }.padding(horizontal = 14.dp, vertical = 9.dp)
        ) {
            Text("อัปเดตเลย", color = C.RedDeep, fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
        }
    }
}

@Composable
private fun ToastHost(m: UiModel, modifier: Modifier) {
    val t = m.toast
    LaunchedEffect(t?.at) {
        if (t != null) {
            delay(if (t.error) 5000 else 2500)
            if (m.toast?.at == t.at) m.toast = null
        }
    }
    AnimatedVisibility(t != null, modifier = modifier.padding(bottom = 14.dp, start = 24.dp, end = 24.dp), enter = fadeIn(), exit = fadeOut()) {
        val last = remember { mutableStateOf(t) }
        if (t != null) last.value = t
        val shown = last.value ?: return@AnimatedVisibility
        Box(
            Modifier
                .clip(RoundedCornerShape(20.dp))
                .background(if (shown.error) Color(0xFF3A1218) else Color(0xFF26262D))
                .border(1.dp, if (shown.error) C.Error else C.Outline, RoundedCornerShape(20.dp))
                .clickable { m.toast = null }
                .padding(horizontal = 18.dp, vertical = 10.dp)
        ) {
            Text(shown.text, color = C.Text, fontSize = 14.sp)
        }
    }
}

// ---- Send ------------------------------------------------------------------------

private val SOURCES = listOf(
    Triple("system", "ทั้งเครื่อง", "ส่งทุกเสียงที่เครื่องนี้เล่นอยู่"),
    Triple("app", "เฉพาะแอป", "ส่งเสียงจากแอปเดียว เช่น เกมหรือเพลง"),
    Triple("mic", "ไมโครโฟน", "ส่งเสียงจากไมโครโฟนของเครื่องนี้"),
)

@Composable
private fun SendTab(m: UiModel, host: Host) {
    val e = m.engine
    Panel {
        Row(verticalAlignment = Alignment.CenterVertically) {
            if (e.sending) Pill("กำลังส่ง", C.RedHi, pulse = true) else Pill("พร้อมส่ง", C.Text2)
            Spacer(Modifier.weight(1f))
            if (e.sending) {
                var s = "${m.sendTo.size} เครื่อง"
                if (e.captureMs > 0) s += " · จับเสียง ${e.captureMs.roundToInt()} ms"
                Text(s, color = C.Text2, fontSize = 12.5.sp)
            }
        }
        Gap(8.dp)
        Visualizer(m.visual, 110.dp, { st, n -> host.scope(send = true, spectrum = st == VizStyle.Bars, n = n) }) { m.toggleVisual() }
        Gap(10.dp)
        if (e.sendError.isNotEmpty()) {
            Text("ปัญหา: ${e.sendError}", color = C.Error, fontSize = 13.sp)
            Gap(6.dp)
        }
        if (e.sending) BigButton("หยุดส่ง", stop = true) { host.stopSending() }
        else BigButton("เริ่มส่งเสียง", stop = false) { host.startSending() }
    }

    Panel {
        Title("แหล่งเสียง")
        Gap(10.dp)
        val sel = SOURCES.indexOfFirst { it.first == m.source }.coerceAtLeast(0)
        Segmented(SOURCES.map { it.second }, sel, enabled = !e.sending) {
            m.source = SOURCES[it].first
            m.save()
        }
        Gap(8.dp)
        Hint(SOURCES[sel].third)
        if (m.source == "app") {
            Gap(8.dp)
            AppPicker(m, host, enabled = !e.sending)
        }
        if (m.source != "mic") {
            Gap(6.dp)
            Hint("ตอนเริ่มส่ง Android จะขออนุญาต \"บันทึกหน้าจอ\" (ใช้แค่เสียง) บางแอปไม่ยอมให้จับเสียง", C.Text3)
        }
        if (e.sending) {
            Gap(6.dp)
            Hint("หยุดส่งก่อนจึงจะเปลี่ยนแหล่งเสียงได้", C.Text3)
        }
    }

    Panel {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Title("ส่งไปที่")
            Spacer(Modifier.weight(1f))
            if (m.sendTo.isNotEmpty()) Text("เลือก ${m.sendTo.size} เครื่อง", color = C.RedHi, fontSize = 12.5.sp)
        }
        Hint("เลือกได้หลายเครื่อง ทุกเครื่องได้ยินพร้อมกัน")
        Gap(10.dp)
        DeviceList(m, m.sendTo)
    }
}

@Composable
private fun AppPicker(m: UiModel, host: Host, enabled: Boolean) {
    var open by remember { mutableStateOf(false) }
    var apps by remember { mutableStateOf<List<AppInfo>?>(null) }
    LaunchedEffect(open) {
        if (open && apps == null) apps = withContext(Dispatchers.Default) { host.apps() }
    }
    val shape = RoundedCornerShape(12.dp)
    Row(
        Modifier
            .fillMaxWidth()
            .clip(shape)
            .background(C.Surface2)
            .border(1.dp, if (open) C.Red else C.Outline, shape)
            .clickable(enabled = enabled) { open = !open }
            .padding(horizontal = 14.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            m.appLabel.ifEmpty { "เลือกแอป…" },
            color = if (m.appLabel.isEmpty()) C.Text3 else C.Text,
            fontSize = 15.sp,
            modifier = Modifier.weight(1f),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        GlyphIcon(Glyph.Chevron, C.Text2, 18.dp)
    }
    if (open && enabled) {
        Gap(6.dp)
        val list = apps
        Column(
            Modifier
                .fillMaxWidth()
                .heightIn(max = 300.dp)
                .clip(shape)
                .background(C.Surface2)
                .verticalScroll(rememberScrollState())
                .padding(vertical = 4.dp)
        ) {
            if (list == null) Hint("กำลังโหลด…", modifier = Modifier.padding(14.dp))
            list?.forEach { app ->
                val on = app.pkg == m.appPkg
                Row(
                    Modifier
                        .fillMaxWidth()
                        .clickable {
                            m.appPkg = app.pkg
                            m.appLabel = app.label
                            m.save()
                            open = false
                        }
                        .padding(horizontal = 14.dp, vertical = 11.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(app.label, color = if (on) C.RedHi else C.Text, fontSize = 15.sp, modifier = Modifier.weight(1f))
                    if (on) CheckCircle(1f)
                }
            }
        }
    }
}

/** Device rows to tick, with an empty state. Used for sending and forwarding. */
@Composable
private fun DeviceList(m: UiModel, chosen: MutableList<String>) {
    val targets = m.targets()
    if (targets.isEmpty() && chosen.isEmpty()) {
        Column(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(C.Surface2).padding(16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text("ยังไม่พบเครื่องอื่น", color = C.Text, fontSize = 15.sp)
            Gap(2.dp)
            Hint("เปิด Stream Sound บนอีกเครื่องที่ต่อ Wi-Fi เดียวกัน แล้วเครื่องจะขึ้นที่นี่เอง")
            Gap(6.dp)
            Text("หรือเพิ่มด้วย IP", color = C.RedHi, fontSize = 14.sp, modifier = Modifier.clickable { m.tab = Tab.Settings }.padding(4.dp))
        }
    }
    targets.forEach { t ->
        val ip = t.addr.substringBefore(':')
        val (detail, color) = when {
            t.manual -> "เพิ่มด้วย IP" to C.Text2
            t.receiving -> ip to C.Text2
            else -> "$ip · ไม่ได้เปิดรับเสียง" to C.Amber
        }
        DeviceRow(t.name, detail, color, chosen.contains(t.addr)) { m.toggle(chosen, t.addr) }
    }
    // Chosen earlier but not around now: still listed so it can be unticked.
    chosen.filter { k -> targets.none { it.addr == k } }.forEach { k ->
        DeviceRow(k.substringBefore(':'), "ไม่พบในเครือข่ายตอนนี้", C.Text3, true) { m.toggle(chosen, k) }
    }
}

// ---- Receive ---------------------------------------------------------------------

@Composable
private fun ReceiveTab(m: UiModel, host: Host) {
    val e = m.engine
    Panel {
        Row(verticalAlignment = Alignment.CenterVertically) {
            when {
                !e.receiving -> Pill("ปิดรับอยู่", C.Text3)
                e.streams.isEmpty() -> Pill("รอเสียง…", C.Text2)
                e.streams.size == 1 -> Pill("กำลังเล่น", C.Green, pulse = true)
                else -> Pill("กำลังเล่น · ${e.streams.size} แหล่ง", C.Green, pulse = true)
            }
            Spacer(Modifier.weight(1f))
            Text("เปิดรับ", color = C.Text2, fontSize = 14.sp)
            Spacer(Modifier.width(10.dp))
            Toggle(e.receiving) { host.setReceiving(it) }
        }
        Gap(8.dp)
        Visualizer(m.visual, 110.dp, { st, n -> host.scope(send = false, spectrum = st == VizStyle.Bars, n = n) }) { m.toggleVisual() }
        Gap(12.dp)
        VolumeRow(m, host)
        if (e.receiverError.isNotEmpty()) {
            Gap(6.dp)
            Hint("เปิดลำโพงไม่ได้ กำลังลองใหม่อัตโนมัติ… (${e.receiverError})", C.Error)
        }
    }

    Panel {
        Title("กำลังรับจาก")
        if (e.streams.isEmpty()) {
            Gap(2.dp)
            Hint(
                when {
                    !e.receiving -> "เปิดรับเสียงก่อน แล้วเครื่องอื่นจะส่งมาที่เครื่องนี้ได้"
                    e.ips.isNotEmpty() -> "ยังไม่มีเสียงเข้ามา ให้อีกเครื่องส่งมาที่ ${e.ips.first()}"
                    else -> "ยังไม่มีเสียงเข้ามา"
                }
            )
        } else {
            Gap(10.dp)
            val now = System.currentTimeMillis()
            e.streams.forEach { s ->
                StreamRow(s, e.outputMs, m.recentStutter(s.id, now), m.expanded == s.id) {
                    m.expanded = if (m.expanded == s.id) null else s.id
                }
            }
            Hint("ไม่รวมเวลาเดินทางใน Wi-Fi (~2–10 ms) · ลำโพง: " + if (e.aaudio) "AAudio" else "AudioTrack", C.Text3)
        }
    }

    Panel {
        Row(Modifier.fillMaxWidth().clickable { m.relayOpen = !m.relayOpen }, verticalAlignment = Alignment.CenterVertically) {
            Title("ส่งต่อ (ต่อเป็นทอด)")
            Spacer(Modifier.weight(1f))
            if (m.forwardTo.isNotEmpty()) Text("${m.forwardTo.size} เครื่อง", color = C.RedHi, fontSize = 12.5.sp)
            Spacer(Modifier.width(8.dp))
            val rot by animateFloatAsState(if (m.relayOpen) 180f else 0f, tween(150), label = "chev")
            GlyphIcon(Glyph.Chevron, C.Text2, 18.dp, Modifier.rotate(rot))
        }
        if (!m.relayOpen) {
            Hint("ส่งเสียงที่รับได้ต่อไปยังเครื่องอื่นทันที เพิ่มหน่วงไม่ถึง 1 ms")
        } else {
            Gap(6.dp)
            ToggleRow("เล่นเสียงที่เครื่องนี้", "ปิดไว้ถ้าเครื่องนี้เป็นแค่ตัวส่งต่อ", e.playLocal) { host.setPlayLocal(it) }
            Gap(6.dp)
            DeviceList(m, m.forwardTo)
            if (e.forwarded > 0) Hint("ส่งต่อแล้ว ${e.forwarded} แพ็กเก็ต", C.Text3)
        }
    }
}

@Composable
private fun VolumeRow(m: UiModel, host: Host) {
    // Live level before volume, from the newest 15 ms of what is playing.
    var level by remember { mutableFloatStateOf(0f) }
    val volume by rememberUpdatedState(m.effectiveVolume)
    LaunchedEffect(Unit) {
        while (true) {
            withFrameNanos { }
            val w = host.scope(send = false, spectrum = false, n = 400)
            val played = if (w == null) 0f else maxOf(w[397], w[398], w[399])
            val pre = if (volume > 0.01f) (played / volume).coerceIn(0f, 1f) else 0f
            level = if (pre > level) pre else level * 0.88f + pre * 0.12f
        }
    }
    Row(verticalAlignment = Alignment.CenterVertically) {
        IconCircle(if (m.muted || m.volume == 0f) Glyph.Muted else Glyph.Speaker) {
            m.muted = !m.muted
            host.setVolume(m.effectiveVolume)
            m.save()
        }
        Spacer(Modifier.width(10.dp))
        VolumeTube(m.volume, 1.5f, if (m.muted) 0f else level, Modifier.weight(1f), onChange = {
            m.volume = it
            if (it > 0f) m.muted = false
            host.setVolume(m.effectiveVolume)
        }, onDone = { m.save() })
        Spacer(Modifier.width(10.dp))
        Text(
            if (m.muted) "ปิด" else "${(m.volume * 100).roundToInt()}%",
            color = if (m.muted) C.Text3 else C.Text,
            fontSize = 14.sp,
            modifier = Modifier.width(44.dp),
        )
    }
}

@Composable
private fun StreamRow(s: StreamInfo, outMs: Double, stutter: Boolean, open: Boolean, onClick: () -> Unit) {
    val total = s.captureMs + s.bufferMs + outMs
    val color = if (stutter) C.Amber else C.Green
    val shape = RoundedCornerShape(12.dp)
    Column(Modifier.fillMaxWidth().padding(bottom = 6.dp).clip(shape).background(if (open) C.Hover else C.Surface2).clickable(onClick = onClick)) {
        Row(Modifier.padding(horizontal = 12.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
            Avatar(s.name, false, ring = C.Green.copy(alpha = 0.25f + 0.75f * s.level.coerceIn(0f, 1f)))
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text(s.name, color = C.Text, fontSize = 15.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text("จาก ${s.from.substringBefore(':')}", color = C.Text2, fontSize = 12.5.sp)
            }
            Box(Modifier.clip(CircleShape).background(color.copy(alpha = 0.14f)).padding(horizontal = 10.dp, vertical = 4.dp)) {
                Text(if (stutter) "${total.roundToInt()} ms · สะดุด" else "${total.roundToInt()} ms", color = color, fontSize = 13.sp)
            }
        }
        if (open) {
            Column(Modifier.padding(start = 60.dp, end = 12.dp, bottom = 10.dp)) {
                val parts = mutableListOf<String>()
                if (s.captureMs > 0) parts.add("จับเสียง ${s.captureMs.roundToInt()}")
                parts.add("บัฟเฟอร์ ${s.bufferMs.roundToInt()}")
                if (outMs > 0) parts.add("ลำโพง ${outMs.roundToInt()}")
                Hint("หน่วงรวม ~${total.roundToInt()} ms = ${parts.joinToString(" + ")}")
                Hint("หาย ${s.lost} · มาช้า ${s.late} · สะดุด ${s.underruns} · ${s.rate / 1000f} kHz")
            }
        }
    }
}

// ---- Settings ----------------------------------------------------------------------

private val MODES = listOf(
    listOf("game", "เกม", "หน่วงต่ำสุด", "เสียงตรงกับภาพที่สุด เสียงที่มาช้าจะถูกข้ามไป ถ้า Wi-Fi สะดุดอาจได้ยินเสียงแตกสั้น ๆ"),
    listOf("balanced", "สมดุล", "แนะนำ", "หน่วงต่ำและไม่สะดุด เหมาะกับการใช้งานทั่วไป"),
    listOf("music", "ฟังเพลง", "เสถียรที่สุด", "เผื่อเวลามากขึ้น ทน Wi-Fi ที่ไม่เสถียรได้ดีที่สุด ไม่ทิ้งเสียงเลย"),
)

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SettingsTab(m: UiModel, host: Host) {
    val e = m.engine
    Panel {
        Title("โหมดเสียง")
        Hint("ใช้กับเสียงที่เครื่องนี้รับ และขนาดแพ็กเก็ตที่เครื่องนี้ส่ง")
        Gap(10.dp)
        MODES.forEach { (id, name, tag, detail) ->
            ModeOption(name, tag, detail, m.latencyMode == id) {
                if (m.latencyMode != id) {
                    m.latencyMode = id
                    host.setMode(id)
                    m.save()
                }
            }
        }
    }

    Panel {
        Title("เครื่องนี้")
        LaunchedEffect(e.name) { if (m.nameInput.isEmpty()) m.nameInput = e.name }
        Gap(8.dp)
        Hint("ชื่อที่เครื่องอื่นเห็น")
        Gap(4.dp)
        val commit = {
            val n = m.nameInput.trim().take(60)
            if (n.isEmpty()) {
                m.nameInput = e.name
            } else if (n != e.name) {
                host.rename(n)
                m.saveName(n)
                m.toast("เปลี่ยนชื่อแล้ว เครื่องอื่นจะเห็นในไม่กี่วินาที")
            }
        }
        Field(m.nameInput, { m.nameInput = it }, "ชื่อเครื่อง", KeyboardType.Text, commit, Modifier.fillMaxWidth())
        Gap(12.dp)
        Hint("IP ของเครื่องนี้ (ให้เครื่องอื่นส่งมาที่นี่)")
        Gap(6.dp)
        if (e.ips.isEmpty()) Text("ยังไม่ได้ต่อ Wi-Fi", color = C.Amber, fontSize = 14.sp)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            e.ips.forEach { ip ->
                Row(
                    Modifier
                        .clip(RoundedCornerShape(10.dp))
                        .background(C.Surface2)
                        .border(1.dp, C.Outline, RoundedCornerShape(10.dp))
                        .clickable {
                            host.copy(ip)
                            m.toast("คัดลอก $ip แล้ว")
                        }
                        .padding(horizontal = 12.dp, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(ip, color = C.Text, fontSize = 15.sp)
                    Spacer(Modifier.width(8.dp))
                    GlyphIcon(Glyph.Copy, C.Text2, 16.dp)
                }
            }
        }
    }

    Panel {
        Title("เพิ่มเครื่องด้วย IP")
        Hint("ใช้เมื่อเครื่องไม่ขึ้นเอง เช่น อยู่คนละวง Wi-Fi หรือเราเตอร์บล็อกการค้นหา")
        Gap(10.dp)
        var input by remember { mutableStateOf("") }
        val add = {
            val a = normalizeAddr(input)
            if (a == null) {
                if (input.isNotBlank()) m.toast("IP ไม่ถูกต้อง ลองพิมพ์แบบ 192.168.1.20", error = true)
            } else {
                if (!m.manual.contains(a)) m.manual.add(a)
                if (!m.sendTo.contains(a)) m.sendTo.add(a)
                m.save()
                input = ""
                m.toast("เพิ่ม ${a.substringBefore(':')} แล้ว และเลือกเป็นปลายทางส่งเสียง")
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Field(input, { input = it }, "192.168.1.20", KeyboardType.Uri, add, Modifier.weight(1f))
            Spacer(Modifier.width(8.dp))
            Box(
                Modifier.height(46.dp).clip(RoundedCornerShape(12.dp)).background(C.Red).clickable { add() }.padding(horizontal = 18.dp),
                contentAlignment = Alignment.Center,
            ) {
                Text("เพิ่ม", color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.SemiBold)
            }
        }
        m.manual.toList().forEach { a ->
            Gap(6.dp)
            Row(
                Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).background(C.Surface2).padding(start = 14.dp, end = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(if (a.endsWith(":$AUDIO_PORT")) a.substringBefore(':') else a, color = C.Text, fontSize = 15.sp, modifier = Modifier.weight(1f))
                Box(
                    Modifier.size(42.dp).clickable {
                        m.manual.remove(a)
                        if (m.engine.peers.none { it.addr == a }) {
                            m.sendTo.remove(a)
                            m.forwardTo.remove(a)
                        }
                        m.save()
                    },
                    contentAlignment = Alignment.Center,
                ) { GlyphIcon(Glyph.Close, C.Text2, 16.dp) }
            }
        }
    }

    Panel {
        Title("ทั่วไป")
        Gap(8.dp)
        Hint("ภาพเสียง")
        Gap(6.dp)
        Segmented(listOf("แท่งความถี่", "คลื่นเสียง"), if (m.visual == VizStyle.Wave) 1 else 0) {
            m.visual = if (it == 1) VizStyle.Wave else VizStyle.Bars
            m.save()
        }
        Gap(10.dp)
        ToggleRow("ให้จอเปิดค้างระหว่างสตรีม", "มือถือหน่วงน้อยที่สุดและไม่หลุดเมื่อจอเปิดอยู่", m.keepScreenOn) {
            m.keepScreenOn = it
            m.save()
        }
        ToggleRow("เปิดรับเสียงทันทีเมื่อเปิดแอป", "เครื่องอื่นส่งมาได้เลยโดยไม่ต้องกดอะไร", m.autoReceive) {
            m.autoReceive = it
            m.save()
        }
    }

    Panel {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Title("อัปเดต")
                Hint("เวอร์ชันนี้: ${m.update.version}")
            }
            Box(
                Modifier
                    .clip(RoundedCornerShape(10.dp))
                    .background(C.Surface2)
                    .border(1.dp, C.Outline, RoundedCornerShape(10.dp))
                    .clickable(enabled = !m.update.busy) { host.checkUpdate() }
                    .padding(horizontal = 14.dp, vertical = 9.dp)
            ) {
                Text("ตรวจสอบอัปเดต", color = if (m.update.busy) C.Text3 else C.Text, fontSize = 14.sp)
            }
        }
        if (m.update.text.isNotEmpty()) {
            Gap(6.dp)
            Hint(m.update.text)
        }
        Gap(10.dp)
        ToggleRow("รับเวอร์ชันเบต้าด้วย", "ได้ของใหม่ก่อน แต่อาจยังไม่เสถียรเท่าเวอร์ชันทางการ", m.betaUpdates) {
            m.betaUpdates = it
            m.save()
            host.checkUpdate()
        }
        Gap(4.dp)
        Hint("แอปตรวจหาเวอร์ชันใหม่จาก GitHub ให้เองตอนเปิดและทุก 5 นาที", C.Text3)
    }

    Box(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(14.dp))
            .border(1.dp, C.Outline, RoundedCornerShape(14.dp))
            .clickable { host.quit() }
            .padding(vertical = 15.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text("ปิดแอปและหยุดทั้งหมด", color = C.Error, fontSize = 15.sp)
    }
    Gap(8.dp)
    Text(
        "Stream Sound · ส่งเสียงข้ามเครื่องในวง LAN",
        color = C.Text3,
        fontSize = 12.sp,
        modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
        textAlign = TextAlign.Center,
    )
}

@Composable
private fun ModeOption(name: String, tag: String, detail: String, on: Boolean, onClick: () -> Unit) {
    val k by animateFloatAsState(if (on) 1f else 0f, tween(140), label = "mode")
    val shape = RoundedCornerShape(12.dp)
    Row(
        Modifier
            .fillMaxWidth()
            .padding(bottom = 8.dp)
            .clip(shape)
            .background(lerp(C.Surface2, C.RedTint, k))
            .border(1.dp, C.Red.copy(alpha = k), shape)
            .clickable(onClick = onClick)
            .padding(14.dp),
    ) {
        Box(Modifier.padding(top = 2.dp).size(20.dp).clip(CircleShape).border(1.8.dp, lerp(C.Text3, C.Red, k), CircleShape), contentAlignment = Alignment.Center) {
            Box(Modifier.size(10.dp * k).clip(CircleShape).background(C.RedHi))
        }
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(name, color = C.Text, fontSize = 15.5.sp, fontWeight = FontWeight.SemiBold)
                Spacer(Modifier.width(8.dp))
                Box(Modifier.clip(CircleShape).background(if (on) C.Red.copy(alpha = 0.22f) else C.Outline).padding(horizontal = 8.dp, vertical = 2.dp)) {
                    Text(tag, color = if (on) C.RedHi else C.Text2, fontSize = 12.sp)
                }
            }
            Gap(3.dp)
            Hint(detail)
        }
    }
}

@Composable
private fun Field(value: String, onChange: (String) -> Unit, hint: String, type: KeyboardType, onDone: () -> Unit, modifier: Modifier) {
    var focused by remember { mutableStateOf(false) }
    val shape = RoundedCornerShape(12.dp)
    BasicTextField(
        value = value,
        onValueChange = onChange,
        singleLine = true,
        textStyle = TextStyle(color = C.Text, fontSize = 15.sp),
        cursorBrush = SolidColor(C.RedHi),
        keyboardOptions = KeyboardOptions(keyboardType = type, imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = { onDone() }),
        modifier = modifier.onFocusChanged {
            if (focused && !it.isFocused) onDone()
            focused = it.isFocused
        },
        decorationBox = { inner ->
            Box(
                Modifier
                    .height(46.dp)
                    .clip(shape)
                    .background(C.Surface2)
                    .border(1.dp, if (focused) C.Red else C.Outline, shape)
                    .padding(horizontal = 14.dp),
                contentAlignment = Alignment.CenterStart,
            ) {
                if (value.isEmpty()) Text(hint, color = C.Text3, fontSize = 15.sp)
                inner()
            }
        },
    )
}
