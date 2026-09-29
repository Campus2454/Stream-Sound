package app.streamsound

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.Settings
import androidx.core.content.FileProvider
import org.json.JSONObject
import java.io.File
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL

/**
 * Self-update from GitHub Releases. CI tags each release `v0.1.<build>` and
 * attaches StreamSound.apk; the app's versionCode is that same build number.
 * Android always asks the user to confirm installing a sideloaded update.
 */
object Updater {
    /**
     * Newest build wins. The source repo is private, so CI also publishes each
     * build to a public releases-only repo the app can read without a login.
     */
    private val APIS = listOf(
        "https://api.github.com/repos/Campus2454/Audio-Streaming-Releases/releases/latest",
        "https://api.github.com/repos/Campus2454/Audio-Streaming/releases/latest",
    )
    private const val ASSET = "StreamSound.apk"

    data class Release(val build: Long, val tag: String, val url: String)

    sealed class Check {
        object UpToDate : Check()
        /** Nothing has been published on GitHub Releases yet. */
        object NoRelease : Check()
        data class Newer(val release: Release) : Check()
    }

    fun currentBuild(ctx: Context): Long =
        ctx.packageManager.getPackageInfo(ctx.packageName, 0).longVersionCode

    private fun open(url: String): HttpURLConnection =
        (URL(url).openConnection() as HttpURLConnection).apply {
            connectTimeout = 10_000
            readTimeout = 30_000
            instanceFollowRedirects = true
            setRequestProperty("User-Agent", "StreamSound-updater")
        }

    /** The latest release at [api], or null when there is none (404, also for a private repo). */
    private fun latest(api: String): JSONObject? {
        val conn = open(api)
        conn.setRequestProperty("Accept", "application/vnd.github+json")
        try {
            val code = conn.responseCode
            if (code == 404) return null
            if (code != 200) throw IOException("GitHub HTTP $code")
            return JSONObject(conn.inputStream.bufferedReader().use { it.readText() })
        } finally {
            conn.disconnect()
        }
    }

    private fun buildOf(o: JSONObject): Long {
        val tag = o.getString("tag_name")
        return tag.substringAfterLast('.').toLongOrNull() ?: throw IOException("unexpected tag $tag")
    }

    /** Compare the newest release with this install. Blocking. */
    fun check(ctx: Context): Check {
        var best: JSONObject? = null
        var error: Exception? = null
        for (api in APIS) {
            try {
                val o = latest(api) ?: continue
                if (best == null || buildOf(o) > buildOf(best)) best = o
            } catch (e: Exception) {
                error = e
            }
        }
        val o = best ?: if (error != null) throw error else return Check.NoRelease
        val tag = o.getString("tag_name")
        val build = buildOf(o)
        if (build <= currentBuild(ctx)) return Check.UpToDate
        val assets = o.getJSONArray("assets")
        for (i in 0 until assets.length()) {
            val a = assets.getJSONObject(i)
            if (a.optString("name") == ASSET) {
                return Check.Newer(Release(build, tag, a.getString("browser_download_url")))
            }
        }
        throw IOException("release $tag has no $ASSET")
    }

    /** Download the apk into the app's cache. Blocking; [progress] gets 0..100. */
    fun download(ctx: Context, release: Release, progress: (Int) -> Unit): File {
        val dir = File(ctx.cacheDir, "updates").apply { mkdirs() }
        val out = File(dir, ASSET)
        val tmp = File(dir, "$ASSET.part")
        val conn = open(release.url)
        try {
            if (conn.responseCode != 200) throw IOException("download HTTP ${conn.responseCode}")
            val total = conn.contentLengthLong
            var done = 0L
            var lastPct = -1
            conn.inputStream.use { input ->
                tmp.outputStream().use { output ->
                    val buf = ByteArray(64 * 1024)
                    while (true) {
                        val n = input.read(buf)
                        if (n < 0) break
                        output.write(buf, 0, n)
                        done += n
                        val pct = if (total > 0) (done * 100 / total).toInt() else -1
                        if (pct != lastPct) {
                            lastPct = pct
                            progress(pct.coerceAtLeast(0))
                        }
                    }
                }
            }
            if ((total > 0 && done != total) || done < 512 * 1024) {
                tmp.delete()
                throw IOException("download incomplete")
            }
            out.delete()
            if (!tmp.renameTo(out)) throw IOException("cannot save update")
            progress(100)
            return out
        } finally {
            conn.disconnect()
        }
    }

    /**
     * Open the system installer for [apk]. Returns false (after opening the
     * right settings page) if the user first has to allow installs from this app.
     */
    fun install(ctx: Context, apk: File): Boolean {
        if (!ctx.packageManager.canRequestPackageInstalls()) {
            ctx.startActivity(
                Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${ctx.packageName}"))
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            )
            return false
        }
        val uri = FileProvider.getUriForFile(ctx, "${ctx.packageName}.files", apk)
        ctx.startActivity(
            Intent(Intent.ACTION_VIEW)
                .setDataAndType(uri, "application/vnd.android.package-archive")
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
        )
        return true
    }
}
