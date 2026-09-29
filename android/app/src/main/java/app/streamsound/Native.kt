package app.streamsound

/** Bridge to the Rust engine (android/rust). Every call is crash-safe on the native side. */
object Native {
    init {
        System.loadLibrary("ssnd_android")
    }

    /** [mode] is "game", "balanced" or "music". */
    external fun create(name: String, mode: String): Long
    external fun destroy(handle: Long)

    /** Returns "" on success, otherwise an error message. */
    external fun startReceiving(handle: Long): String
    external fun stopReceiving(handle: Long)

    /** Plays received audio through AAudio. Returns "" on success, otherwise why not. */
    external fun startPlayer(handle: Long): String
    external fun stopPlayer(handle: Long)
    /** True when the AAudio player gave up; play through AudioTrack and [render] instead. */
    external fun playerFailed(handle: Long): Boolean
    external fun setOutputLatency(handle: Long, ms: Float)
    external fun setCaptureLatency(handle: Long, ms: Float)

    /** Fills [out] with [frames] frames of interleaved audio. */
    external fun render(handle: Long, out: FloatArray, frames: Int, rate: Int, channels: Int)

    /** [dests] is a comma-separated list of "ip:port". Returns "" on success. */
    external fun startSending(handle: Long, dests: String): String
    external fun stopSending(handle: Long)
    external fun pushCapture(handle: Long, data: FloatArray, len: Int, rate: Int, channels: Int)

    external fun setSendDests(handle: Long, dests: String)
    external fun setForward(handle: Long, dests: String)
    external fun setPlayLocal(handle: Long, on: Boolean)
    external fun setVolume(handle: Long, volume: Float)
    external fun setMode(handle: Long, mode: String)

    /** Snapshot of peers, incoming streams and sender status as JSON. */
    external fun stateJson(handle: Long): String
}
