//! Thread priorities for the audio path. Everything here is best effort:
//! if the OS refuses, the thread simply keeps its normal priority.

#[derive(Clone, Copy)]
pub enum Priority {
    /// Feeds or drains a sound card.
    Audio,
    /// Receives packets and fills jitter buffers.
    Network,
}

/// Raise the calling thread's priority so a busy system can't delay audio.
pub fn boost_current_thread(p: Priority) {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        // Android apps may use -19 (THREAD_PRIORITY_URGENT_AUDIO) for their
        // own threads; desktop Linux usually refuses negative values without
        // rtkit, which is harmless.
        let nice = match p {
            Priority::Audio => -19,
            Priority::Network => -16,
        };
        // SAFETY: plain syscalls on the current thread.
        unsafe {
            let tid = libc::gettid();
            let _ = libc::setpriority(libc::PRIO_PROCESS, tid as libc::id_t, nice);
        }
    }
    #[cfg(windows)]
    {
        use windows::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_HIGHEST, THREAD_PRIORITY_TIME_CRITICAL,
        };
        let level = match p {
            Priority::Audio => THREAD_PRIORITY_TIME_CRITICAL,
            Priority::Network => THREAD_PRIORITY_HIGHEST,
        };
        // SAFETY: GetCurrentThread returns a pseudo-handle valid for this thread.
        unsafe {
            let _ = SetThreadPriority(GetCurrentThread(), level);
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "android", windows)))]
    let _ = p;
}
