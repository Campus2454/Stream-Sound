package app.streamsound

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.Settings
import androidx.core.content.FileProvider
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
     * The repo's web address; it must be public. Updates are found through
     * github.com pages, not the REST API, because the API allows only 60
     * anonymous requests an hour per home connection, which a few devices
     * checking every 5 minutes would use up.
     */
    private const val REPO = "https://github.com/Campus2454/Stream-Sound"
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

    /** Release tags linked from [text] ("…/releases/tag/v0.2.1…"). */
    private fun tagsIn(text: String): List<String> =
        Regex("""/releases/tag/([A-Za-z0-9.]+)""").findAll(text).map { it.groupValues[1] }.toList()

    /**
     * Tags of the newest releases. With betas on, from the releases feed (the
     * latest 10, pre-releases included). Otherwise from where /releases/latest
     * redirects to, which is the newest official release.
     */
    private fun releaseTags(beta: Boolean): List<String> {
        val conn = open(if (beta) "$REPO/releases.atom" else "$REPO/releases/latest")
        if (!beta) conn.instanceFollowRedirects = false
        try {
            val code = conn.responseCode
            if (code == 404) return emptyList()
            if (!beta) return tagsIn(conn.getHeaderField("Location") ?: "")
            if (code != 200) throw IOException("GitHub HTTP $code")
            return tagsIn(conn.inputStream.bufferedReader().use { it.readText() })
        } finally {
            conn.disconnect()
        }
    }

    /** Compare the newest release (official only unless [beta]) with this install. Blocking. */
    fun check(ctx: Context, beta: Boolean): Check {
        val newest = releaseTags(beta).mapNotNull { Version.parse(it) }.filter { beta || !it.isBeta }.maxOrNull()
            ?: return Check.NoRelease
        val cur = current(ctx)
        if (cur != null && newest <= cur) return Check.UpToDate
        val tag = "v$newest"
        return Check.Newer(Release(newest, tag, "$REPO/releases/download/$tag/$ASSET"))
    }

    /** The release's files are still being uploaded; the next check picks it up. */
    class NotUploadedYet : IOException("the new version's files are still being uploaded")

    /** Download the apk into the app's cache. Blocking; [progress] gets 0..100. */
    fun download(ctx: Context, release: Release, progress: (Int) -> Unit): File {
        val dir = File(ctx.cacheDir, "updates").apply { mkdirs() }
        val out = File(dir, ASSET)
        val tmp = File(dir, "$ASSET.part")
        val conn = open(release.url)
        try {
            if (conn.responseCode == 404) throw NotUploadedYet()
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
