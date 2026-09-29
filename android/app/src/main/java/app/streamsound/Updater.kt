package app.streamsound

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.Settings
import androidx.core.content.FileProvider
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL

/**
 * Self-update from GitHub Releases. Releases are tagged `vX.Y` (official) or
 * `vX.Y.Z` (beta, the Z-th change on main after vX.Y); see .github/version.sh.
 * CI sets the app's versionName to that version and its versionCode to match.
 * Android always asks the user to confirm installing a sideloaded update.
 */
object Updater {
    /**
     * The repo must be public: GitHub answers 404 to anonymous requests for a
     * private one. Installs from before the rename from Audio-Streaming reach
     * it through GitHub's redirect.
     */
    private const val API = "https://api.github.com/repos/Campus2454/Stream-Sound"
    private const val ASSET = "StreamSound.apk"

    /** `X.Y` (official) or `X.Y.Z` (beta); 0.2 < 0.2.1 < 0.2.2 < 0.3. */
    data class Version(val major: Int, val minor: Int, val beta: Int?) : Comparable<Version> {
        val isBeta get() = beta != null

        override fun compareTo(other: Version) =
            compareValuesBy(this, other, { it.major }, { it.minor }, { it.beta ?: -1 })

        override fun toString() = if (beta == null) "$major.$minor" else "$major.$minor.$beta"

        companion object {
            private val PATTERN = Regex("""v?(\d{1,9})\.(\d{1,9})(?:\.(\d{1,9}))?""")

            /** Parses "v0.2", "0.2" or "v0.2.3"; null for anything else. */
            fun parse(s: String?): Version? {
                val m = PATTERN.matchEntire(s?.trim() ?: return null) ?: return null
                val (x, y, z) = m.destructured
                return Version(x.toInt(), y.toInt(), z.takeIf { it.isNotEmpty() }?.toInt())
            }
        }
    }

    data class Release(val version: Version, val tag: String, val url: String)

    sealed class Check {
        object UpToDate : Check()
        /** No release this install may take has been published yet. */
        object NoRelease : Check()
        data class Newer(val release: Release) : Check()
    }

    /** This install's version; null for local builds, which take any release. */
    fun current(ctx: Context): Version? =
        Version.parse(ctx.packageManager.getPackageInfo(ctx.packageName, 0).versionName)

    private fun open(url: String): HttpURLConnection =
        (URL(url).openConnection() as HttpURLConnection).apply {
            connectTimeout = 10_000
            readTimeout = 30_000
            instanceFollowRedirects = true
            setRequestProperty("User-Agent", "StreamSound-updater")
        }

    /** The response body, or null on 404 (no release yet, also for a private repo). */
    private fun get(url: String): String? {
        val conn = open(url)
        conn.setRequestProperty("Accept", "application/vnd.github+json")
        try {
            val code = conn.responseCode
            if (code == 404) return null
            if (code != 200) throw IOException("GitHub HTTP $code")
            return conn.inputStream.bufferedReader().use { it.readText() }
        } finally {
            conn.disconnect()
        }
    }

    /**
     * Compare the newest release (official only unless [beta]) with this
     * install. Betas are GitHub pre-releases, which /releases/latest leaves
     * out, so with betas on the newest few releases are listed instead. Blocking.
     */
    fun check(ctx: Context, beta: Boolean): Check {
        val body = get(if (beta) "$API/releases?per_page=20" else "$API/releases/latest") ?: return Check.NoRelease
        val list = if (beta) JSONArray(body) else JSONArray().put(JSONObject(body))
        var best: Release? = null
        for (i in 0 until list.length()) {
            val o = list.getJSONObject(i)
            if (o.optBoolean("draft")) continue
            val tag = o.optString("tag_name")
            val version = Version.parse(tag) ?: continue
            if (version.isBeta && !beta) continue
            // Skips a release whose files are still being uploaded.
            val url = assetUrl(o) ?: continue
            if (best == null || version > best.version) best = Release(version, tag, url)
        }
        val newest = best ?: return Check.NoRelease
        val cur = current(ctx)
        if (cur != null && newest.version <= cur) return Check.UpToDate
        return Check.Newer(newest)
    }

    private fun assetUrl(release: JSONObject): String? {
        val assets = release.optJSONArray("assets") ?: return null
        for (i in 0 until assets.length()) {
            val a = assets.getJSONObject(i)
            if (a.optString("name") == ASSET) return a.optString("browser_download_url").ifEmpty { null }
        }
        return null
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
