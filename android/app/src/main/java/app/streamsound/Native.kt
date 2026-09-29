package app.streamsound

/** Bridge to the Rust engine (android/rust). Every call is crash-safe on the native side. */
object Native {
    init {
        System.loadLibrary("ssnd_android")
    }

    external fun create(name: String, latencyMs: Float): Long
    external fun destroy(handle: Long)

    /** Returns "" on success, otherwise an error message. */
    external fun startReceiving(handle: Long): String
    external fun stopReceiving(handle: Long)

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
    external fun setLatency(handle: Long, ms: Float)

    /** Snapshot of peers, incoming streams and sender status as JSON. */
    external fun stateJson(handle: Long): String
}
