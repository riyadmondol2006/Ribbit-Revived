package com.riyadm.ribbit.revived

import android.content.Context
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL
import kotlin.concurrent.thread

/**
 * Looks for a newer Ribbit Revived on GitHub Releases. The only network traffic in the app:
 * one anonymous GET to api.github.com, at most every [INTERVAL_MS], and only when the engine
 * asks (Options > Updates is on). The result is cached so the menu banner shows offline too.
 */
class UpdateChecker(context: Context, private val onNewer: (version: String) -> Unit) {

    private val prefs = context.getSharedPreferences("update_check", Context.MODE_PRIVATE)

    /** Installed versionName, e.g. "1.2.0". */
    val current: String = runCatching {
        context.packageManager.getPackageInfo(context.packageName, 0).versionName
    }.getOrNull() ?: "0"

    @Volatile private var running = false

    /** Where the update button goes: the newest APK if the release has one, else the release page. */
    val downloadUrl: String get() = prefs.getString(KEY_URL, null) ?: RELEASES_PAGE

    fun check() {
        prefs.getString(KEY_VERSION, null)?.let { if (isNewer(it, current)) onNewer(it) }
        val now = System.currentTimeMillis()
        if (running || now - prefs.getLong(KEY_CHECKED, 0) < INTERVAL_MS) return
        running = true
        thread(name = "update-check", isDaemon = true) {
            try {
                fetch(now)
            } catch (_: Exception) {
                // offline or GitHub unreachable: try again next time the game comes to the front
            } finally {
                running = false
            }
        }
    }

    private fun fetch(now: Long) {
        val conn = URL(LATEST_API).openConnection() as HttpURLConnection
        try {
            conn.connectTimeout = 8000
            conn.readTimeout = 8000
            conn.setRequestProperty("Accept", "application/vnd.github+json")
            conn.setRequestProperty("User-Agent", "RibbitRevived/$current")
            when (conn.responseCode) {
                200 -> {
                    val json = JSONObject(conn.inputStream.bufferedReader().use { it.readText() })
                    val version = json.getString("tag_name").removePrefix("v").trim()
                    var url = json.optString("html_url", RELEASES_PAGE)
                    val assets = json.optJSONArray("assets")
                    if (assets != null) {
                        for (i in 0 until assets.length()) {
                            val a = assets.getJSONObject(i)
                            if (a.optString("name").endsWith(".apk")) {
                                url = a.getString("browser_download_url")
                                break
                            }
                        }
                    }
                    prefs.edit().putString(KEY_VERSION, version).putString(KEY_URL, url).putLong(KEY_CHECKED, now).apply()
                    if (isNewer(version, current)) onNewer(version)
                }
                404 -> prefs.edit().putLong(KEY_CHECKED, now).apply() // no release published yet
            }
        } finally {
            conn.disconnect()
        }
    }

    companion object {
        private const val REPO = "riyadmondol2006/Ribbit-Revived"
        private const val LATEST_API = "https://api.github.com/repos/$REPO/releases/latest"
        private const val RELEASES_PAGE = "https://github.com/$REPO/releases/latest"
        private const val INTERVAL_MS = 6 * 60 * 60 * 1000L
        private const val KEY_VERSION = "latest_version"
        private const val KEY_URL = "latest_url"
        private const val KEY_CHECKED = "checked_at"

        /** Numeric comparison of dotted versions: 1.10.0 > 1.9.2, 1.2 == 1.2.0. */
        fun isNewer(candidate: String, installed: String): Boolean {
            fun parts(v: String) = v.split('.', '-', '+').map { p -> p.takeWhile { it.isDigit() }.toIntOrNull() ?: 0 }
            val a = parts(candidate)
            val b = parts(installed)
            for (i in 0 until maxOf(a.size, b.size)) {
                val x = a.getOrElse(i) { 0 }
                val y = b.getOrElse(i) { 0 }
                if (x != y) return x > y
            }
            return false
        }
    }
}
