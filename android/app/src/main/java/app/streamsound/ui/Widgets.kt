package app.streamsound.ui

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sin
import kotlin.math.sqrt

/** Colours from docs/DESIGN.md. */
object C {
    val Bg = Color(0xFF0A0A0C)
    val Surface = Color(0xFF141417)
    val Surface2 = Color(0xFF1D1D22)
    val Hover = Color(0xFF23232A)
    val Outline = Color(0xFF2A2A31)
    val Red = Color(0xFFE62639)
    val RedHi = Color(0xFFFF5A6A)
    val RedDeep = Color(0xFF8C1320)
    val RedTint = Color(0xFF2A1519)
    val Text = Color(0xFFF4F4F6)
    val Text2 = Color(0xFFA3A3AD)
    val Text3 = Color(0xFF6E6E78)
    val Green = Color(0xFF3DDC84)
    val Amber = Color(0xFFFFB020)
    val Error = Color(0xFFFF6B6B)
}

@Composable
fun Panel(content: @Composable ColumnScope.() -> Unit) {
    val shape = RoundedCornerShape(16.dp)
    Column(
        Modifier
            .fillMaxWidth()
            .padding(bottom = 12.dp)
            .clip(shape)
            .background(C.Surface)
            .border(1.dp, C.Outline, shape)
            .padding(16.dp),
        content = content,
    )
}

@Composable
fun Title(text: String, modifier: Modifier = Modifier) {
    Text(text, color = C.Text, fontSize = 17.sp, fontWeight = FontWeight.SemiBold, modifier = modifier)
}

@Composable
fun Hint(text: String, color: Color = C.Text2, modifier: Modifier = Modifier) {
    Text(text, color = color, fontSize = 13.sp, lineHeight = 18.sp, modifier = modifier)
}

@Composable
fun Gap(h: Dp) = Spacer(Modifier.height(h))

/** Status pill with a dot that can pulse. */
@Composable
fun Pill(text: String, color: Color, pulse: Boolean = false) {
    val t = rememberInfiniteTransition(label = "pulse")
    val a by t.animateFloat(1f, 0.3f, infiniteRepeatable(tween(800), RepeatMode.Reverse), label = "dot")
    Row(
        Modifier.clip(CircleShape).background(color.copy(alpha = 0.14f)).padding(horizontal = 11.dp, vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(7.dp).clip(CircleShape).background(color.copy(alpha = if (pulse) a else 1f)))
        Spacer(Modifier.width(7.dp))
        Text(text, color = color, fontSize = 13.sp)
    }
}

@Composable
fun Toggle(on: Boolean, onChange: (Boolean) -> Unit) {
    val k by animateFloatAsState(if (on) 1f else 0f, tween(140), label = "toggle")
    Box(
        Modifier
            .size(46.dp, 26.dp)
            .clip(CircleShape)
            .background(lerp(C.Surface2, C.Red, k))
            .border(1.dp, lerp(C.Outline, C.Red, k), CircleShape)
            .clickable { onChange(!on) },
    ) {
        Box(Modifier.offset(x = 3.dp + 20.dp * k, y = 3.dp).size(20.dp).clip(CircleShape).background(Color.White))
    }
}

@Composable
fun ToggleRow(label: String, detail: String?, on: Boolean, onChange: (Boolean) -> Unit) {
    Row(
        Modifier.fillMaxWidth().clickable { onChange(!on) }.padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(label, color = C.Text, fontSize = 15.sp)
            if (detail != null) Hint(detail)
        }
        Spacer(Modifier.width(12.dp))
        Toggle(on, onChange)
    }
}

/** Equal-width segments with a sliding red selection. */
@Composable
fun Segmented(labels: List<String>, selected: Int, enabled: Boolean = true, onSelect: (Int) -> Unit) {
    val pos by animateFloatAsState(selected.toFloat(), tween(160), label = "seg")
    BoxWithConstraints(Modifier.fillMaxWidth().height(40.dp).clip(RoundedCornerShape(12.dp)).background(C.Surface2)) {
        val w = maxWidth / labels.size
        Box(
            Modifier
                .offset(x = w * pos)
                .width(w)
                .fillMaxHeight()
                .padding(3.dp)
                .clip(RoundedCornerShape(9.dp))
                .background(if (enabled) C.Red else C.RedDeep)
        )
        Row(Modifier.fillMaxSize()) {
            labels.forEachIndexed { i, l ->
                Box(
                    Modifier.weight(1f).fillMaxHeight().clickable(enabled = enabled) { onSelect(i) },
                    contentAlignment = Alignment.Center,
                ) {
                    Text(l, color = if (i == selected) C.Text else C.Text2, fontSize = 14.sp, maxLines = 1)
                }
            }
        }
    }
}

/** Full-width call to action: red gradient to start, dark red outline to stop. */
@Composable
fun BigButton(text: String, stop: Boolean, onClick: () -> Unit) {
    val shape = RoundedCornerShape(14.dp)
    val fill = if (stop) {
        Modifier.background(C.RedDeep.copy(alpha = 0.55f), shape).border(1.5.dp, C.Red, shape)
    } else {
        Modifier.background(Brush.horizontalGradient(listOf(C.Red, C.RedHi)), shape)
    }
    Row(
        Modifier.fillMaxWidth().height(54.dp).clip(shape).then(fill).clickable(onClick = onClick),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Canvas(Modifier.size(15.dp)) {
            if (stop) {
                drawRoundRect(Color.White, cornerRadius = CornerRadius(3.dp.toPx()))
            } else {
                val p = Path().apply {
                    moveTo(size.width * 0.12f, 0f)
                    lineTo(size.width, size.height / 2)
                    lineTo(size.width * 0.12f, size.height)
                    close()
                }
                drawPath(p, Color.White)
            }
        }
        Spacer(Modifier.width(10.dp))
        Text(text, color = Color.White, fontSize = 17.sp, fontWeight = FontWeight.SemiBold)
    }
}

/** First letter, skipping Thai leading vowels (เ แ โ ใ ไ); "IP" for an address. */
fun initial(name: String): String {
    if (name.firstOrNull()?.isDigit() == true && name.contains('.')) return "IP"
    return name.firstOrNull { it.isLetterOrDigit() && it !in '\u0E40'..'\u0E44' }?.uppercase() ?: "?"
}

@Composable
fun Avatar(name: String, on: Boolean, ring: Color? = null) {
    Box(
        Modifier
            .size(36.dp)
            .clip(CircleShape)
            .background(if (on) C.Red else C.Outline)
            .then(if (ring != null) Modifier.border(2.5.dp, ring, CircleShape) else Modifier),
        contentAlignment = Alignment.Center,
    ) {
        val i = initial(name)
        Text(i, color = Color.White, fontSize = if (i.length > 1) 12.sp else 16.sp, fontWeight = FontWeight.SemiBold)
    }
}

@Composable
fun CheckCircle(k: Float) {
    Canvas(Modifier.size(22.dp)) {
        val r = size.minDimension / 2
        drawCircle(C.Red.copy(alpha = k), r)
        drawCircle(lerp(C.Text3, C.Red, k), r - 0.75.dp.toPx(), style = Stroke(1.5.dp.toPx()))
        if (k > 0.01f) {
            val s = size.width / 22f
            val c = center
            val p = Path().apply {
                moveTo(c.x - 5 * s, c.y)
                lineTo(c.x - 1.5f * s, c.y + 3.5f * s)
                lineTo(c.x + 5 * s, c.y - 3.5f * s)
            }
            drawPath(p, Color.White.copy(alpha = k), style = Stroke(2.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round))
        }
    }
}

/** A device in a pick list: avatar, name and detail, check circle. */
@Composable
fun DeviceRow(name: String, detail: String, detailColor: Color, selected: Boolean, onClick: () -> Unit) {
    val k by animateFloatAsState(if (selected) 1f else 0f, tween(140), label = "row")
    val shape = RoundedCornerShape(12.dp)
    Row(
        Modifier
            .fillMaxWidth()
            .padding(bottom = 6.dp)
            .clip(shape)
            .background(lerp(C.Surface2, C.RedTint, k))
            .border(1.dp, C.Red.copy(alpha = k), shape)
            .clickable(onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Avatar(name, selected)
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(name, color = C.Text, fontSize = 15.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(detail, color = detailColor, fontSize = 12.5.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        Spacer(Modifier.width(8.dp))
        CheckCircle(k)
    }
}

// ---- visualizer ---------------------------------------------------------------

private fun shaped(style: VizStyle, raw: FloatArray?, n: Int): FloatArray =
    if (raw == null) FloatArray(n) else if (style == VizStyle.Wave) FloatArray(n) { sqrt(raw[it].coerceIn(0f, 1f)) } else raw

/**
 * Mirrored bars around a centre line. [data] gives 0..1 per column for the
 * style (spectrum bands or the 2-second waveform), or null when idle. Tap to
 * switch style.
 */
@Composable
fun Visualizer(style: VizStyle, height: Dp, data: (VizStyle, Int) -> FloatArray?, onClick: () -> Unit) {
    val n = if (style == VizStyle.Bars) 32 else 90
    val source by rememberUpdatedState(data)
    var bars by remember(style) { mutableStateOf(shaped(style, source(style, n), n)) }
    LaunchedEffect(style) {
        var last = 0L
        while (true) {
            withFrameNanos { now ->
                val dt = if (last == 0L) 0f else min((now - last) / 1e9f, 0.1f)
                last = now
                val target = shaped(style, source(style, n), n)
                val prev = bars
                // Bars rise instantly and fall at 2.5 heights per second; the waveform scrolls as is.
                bars = if (style == VizStyle.Wave) target else FloatArray(n) { max(target[it], prev[it] - dt * 2.5f) }
            }
        }
    }
    Canvas(
        Modifier
            .fillMaxWidth()
            .height(height)
            .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null, onClick = onClick)
    ) {
        val gap = (if (style == VizStyle.Bars) 4.dp else 1.5.dp).toPx()
        val w = (size.width - gap * (n - 1)) / n
        val cy = size.height / 2
        val half = cy - 2f
        val r = min(w / 2, 4.dp.toPx())
        val grad = Brush.verticalGradient(listOf(C.RedHi, C.Red), startY = cy - half, endY = cy)
        val minH = 1.dp.toPx()
        for (i in 0 until n) {
            val v = bars[i]
            val x = i * (w + gap)
            val h = max(v * half, minH)
            if (v < 0.03f) {
                drawRect(C.Text3.copy(alpha = 0.7f), Offset(x, cy - minH), Size(w, minH * 2))
                continue
            }
            val up = Path().apply {
                addRoundRect(RoundRect(Rect(x, cy - h, x + w, cy), topLeft = CornerRadius(r), topRight = CornerRadius(r)))
            }
            drawPath(up, grad)
            val down = Path().apply {
                addRoundRect(RoundRect(Rect(x, cy, x + w, cy + h), bottomLeft = CornerRadius(r), bottomRight = CornerRadius(r)))
            }
            drawPath(down, C.Red, alpha = 0.3f)
        }
    }
}

// ---- volume tube --------------------------------------------------------------

/** Loudest volume the sliders reach (200 %). */
const val MAX_VOLUME = 2f
/** Share of a volume slider's length that covers 0–100 %; the rest is 100–200 %. */
private const val UNITY_POS = 0.75f

/** Where a volume (0..2) sits along a slider (0..1). */
fun volumeToPos(v: Float): Float {
    val x = v.coerceIn(0f, MAX_VOLUME)
    return if (x <= 1f) x * UNITY_POS else UNITY_POS + (x - 1f) / (MAX_VOLUME - 1f) * (1f - UNITY_POS)
}

/** The volume at a point along a slider (0..1). */
fun posToVolume(p: Float): Float {
    val x = p.coerceIn(0f, 1f)
    return if (x <= UNITY_POS) x / UNITY_POS else 1f + (x - UNITY_POS) / (1f - UNITY_POS) * (MAX_VOLUME - 1f)
}

private fun volumeAt(x: Float, width: Int, r: Float): Float {
    val at = ((x - r) / (width - 2 * r)).coerceIn(0f, 1f)
    return if (abs(at - UNITY_POS) < 0.02f) 1f else posToVolume(at) // gentle snap to 100 %
}

/**
 * Volume control drawn on the level meter: the knob sets the volume (0–200 %,
 * with 100 % three quarters along) and the live [level] (before volume, 0..1)
 * fills the tube up to the knob.
 */
@Composable
fun VolumeTube(
    volume: Float,
    level: Float,
    modifier: Modifier,
    height: Dp = 30.dp,
    onChange: (Float) -> Unit,
    onDone: () -> Unit,
) {
    val change by rememberUpdatedState(onChange)
    val done by rememberUpdatedState(onDone)
    val knob = height * 0.37f
    Canvas(
        modifier
            .height(height)
            .pointerInput(Unit) {
                detectTapGestures { p ->
                    change(volumeAt(p.x, size.width, knob.toPx()))
                    done()
                }
            }
            .pointerInput(Unit) {
                detectHorizontalDragGestures(onDragEnd = { done() }) { c, _ ->
                    c.consume()
                    change(volumeAt(c.position.x, size.width, knob.toPx()))
                }
            }
    ) {
        val r = knob.toPx()
        val x0 = r
        val x1 = size.width - r
        val kx = x0 + (x1 - x0) * volumeToPos(volume)
        val track = size.height - 2.dp.toPx()
        val top = (size.height - track) / 2
        drawRoundRect(C.Surface2, Offset(0f, top), Size(size.width, track), CornerRadius(track / 2))
        drawRoundRect(C.Outline, Offset(0f, top), Size(size.width, track), CornerRadius(track / 2), style = Stroke(1.dp.toPx()))
        val pad = track * 0.14f
        val ih = track - 2 * pad
        val iy = top + pad
        drawRoundRect(C.Red.copy(alpha = 0.14f), Offset(pad, iy), Size(max(kx - pad, ih), ih), CornerRadius(ih / 2))
        val lx = pad + (kx - pad) * level.coerceIn(0f, 1f)
        if (lx > pad + 2f) {
            drawRoundRect(
                brush = Brush.horizontalGradient(listOf(C.Red, C.RedHi), startX = pad, endX = max(lx, pad + ih)),
                topLeft = Offset(pad, iy),
                size = Size(max(lx - pad, ih), ih),
                cornerRadius = CornerRadius(ih / 2),
            )
        }
        val x100 = x0 + (x1 - x0) * UNITY_POS
        val inset = track * 0.23f
        drawLine(Color.White.copy(alpha = 0.28f), Offset(x100, top + inset), Offset(x100, top + track - inset), 1.5.dp.toPx())
        val c = Offset(kx, size.height / 2)
        drawCircle(Color.Black.copy(alpha = 0.35f), r + 1.5.dp.toPx(), c + Offset(0f, 1.5.dp.toPx()))
        drawCircle(C.Red, r, c)
        drawCircle(Color.White, r * 0.73f, c)
    }
}

// ---- icons (drawn, so every platform looks the same) ---------------------------

enum class Glyph { Send, Receive, Settings, Speaker, Muted, Close, Copy, Refresh, Chevron }

private fun DrawScope.arc(center: Offset, r: Float, startDeg: Float, sweepDeg: Float, stroke: Stroke, color: Color) {
    drawArc(color, startDeg, sweepDeg, false, Offset(center.x - r, center.y - r), Size(2 * r, 2 * r), style = stroke)
}

fun DrawScope.glyph(g: Glyph, color: Color) {
    val s = size.minDimension / 20f
    val c = center
    val st = Stroke(2f * s, cap = StrokeCap.Round, join = StrokeJoin.Round)
    fun p(x: Float, y: Float) = Offset(c.x + x * s, c.y + y * s)
    when (g) {
        Glyph.Send -> {
            drawCircle(color, 2.5f * s, p(0f, 3f))
            arc(p(0f, 3f), 6f * s, -135f, 90f, st, color)
            arc(p(0f, 3f), 10.5f * s, -135f, 90f, st, color)
        }
        Glyph.Receive -> {
            arc(p(0f, 1f), 7.5f * s, 180f, 180f, st, color)
            for (dx in listOf(-7.5f, 7.5f)) {
                drawRoundRect(color, p(dx - 2.25f, 0f), Size(4.5f * s, 8f * s), CornerRadius(2f * s))
            }
        }
        Glyph.Settings -> {
            for (i in 0 until 8) {
                val a = i * PI.toFloat() / 4f
                drawLine(color, p(cos(a) * 5.5f, sin(a) * 5.5f), p(cos(a) * 9f, sin(a) * 9f), 3.2f * s)
            }
            drawCircle(color, 6f * s, c, style = Stroke(2.2f * s))
        }
        Glyph.Speaker, Glyph.Muted -> {
            val body = Path().apply {
                moveTo(c.x - 8 * s, c.y - 3 * s); lineTo(c.x - 4 * s, c.y - 3 * s); lineTo(c.x + 1 * s, c.y - 7.5f * s)
                lineTo(c.x + 1 * s, c.y + 7.5f * s); lineTo(c.x - 4 * s, c.y + 3 * s); lineTo(c.x - 8 * s, c.y + 3 * s); close()
            }
            drawPath(body, color)
            if (g == Glyph.Muted) {
                drawLine(color, p(4.5f, -3.5f), p(11f, 3.5f), 1.8f * s, StrokeCap.Round)
                drawLine(color, p(4.5f, 3.5f), p(11f, -3.5f), 1.8f * s, StrokeCap.Round)
            } else {
                arc(p(1f, 0f), 5f * s, -45f, 90f, st, color)
                arc(p(1f, 0f), 9.5f * s, -45f, 90f, st, color)
            }
        }
        Glyph.Close -> {
            drawLine(color, p(-5f, -5f), p(5f, 5f), 2f * s, StrokeCap.Round)
            drawLine(color, p(5f, -5f), p(-5f, 5f), 2f * s, StrokeCap.Round)
        }
        Glyph.Copy -> {
            drawRoundRect(color.copy(alpha = 0.6f), p(-7f, -7f), Size(10f * s, 10f * s), CornerRadius(2.5f * s), style = Stroke(1.6f * s))
            drawRoundRect(C.Surface2, p(-3f, -3f), Size(10f * s, 10f * s), CornerRadius(2.5f * s))
            drawRoundRect(color, p(-3f, -3f), Size(10f * s, 10f * s), CornerRadius(2.5f * s), style = Stroke(1.6f * s))
        }
        Glyph.Refresh -> {
            arc(c, 7f * s, -60f, 300f, st, color)
            val a = (-60f / 180f * PI).toFloat()
            val tip = p(cos(a) * 7f, sin(a) * 7f)
            val arrow = Path().apply {
                moveTo(tip.x + 4f * s, tip.y - 1f * s); lineTo(tip.x - 1.5f * s, tip.y - 3.5f * s); lineTo(tip.x - 0.5f * s, tip.y + 3f * s); close()
            }
            drawPath(arrow, color)
        }
        Glyph.Chevron -> {
            val path = Path().apply { moveTo(c.x - 5 * s, c.y - 2.5f * s); lineTo(c.x, c.y + 2.5f * s); lineTo(c.x + 5 * s, c.y - 2.5f * s) }
            drawPath(path, color, style = st)
        }
    }
}

@Composable
fun GlyphIcon(g: Glyph, color: Color, size: Dp = 22.dp, modifier: Modifier = Modifier) {
    Canvas(modifier.size(size)) { glyph(g, color) }
}

/** Round icon button. */
@Composable
fun IconCircle(g: Glyph, size: Dp = 38.dp, onClick: () -> Unit) {
    Box(
        Modifier.size(size).clip(CircleShape).background(C.Surface2).clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        GlyphIcon(g, C.Text, size * 0.55f)
    }
}

/** Logo mark: gradient rounded square with five white equalizer bars. */
@Composable
fun Logo(size: Dp) {
    Canvas(Modifier.size(size)) {
        val w = this.size.width
        drawRoundRect(Brush.horizontalGradient(listOf(C.Red, C.RedHi)), cornerRadius = CornerRadius(w * 0.24f))
        // Same bars as the app icon.
        val heights = listOf(0.205f, 0.41f, 0.586f, 0.352f, 0.234f)
        val bw = w * 0.09f
        val gap = w * 0.062f
        val total = 5 * bw + 4 * gap
        heights.forEachIndexed { i, h ->
            val x = w / 2 - total / 2 + i * (bw + gap)
            drawRoundRect(Color.White, Offset(x, w / 2 - h * w / 2), Size(bw, h * w), CornerRadius(bw / 2))
        }
    }
}
