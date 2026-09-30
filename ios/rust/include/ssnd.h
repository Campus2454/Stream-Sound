// C interface of the Stream Sound engine for the iOS app and its
// screen-broadcast extension. Implemented in ios/rust/src/lib.rs.
//
// Strings returned by these functions are owned by the caller and must be
// released with ssnd_free_string. Functions returning `char *` for errors
// return NULL on success.

#ifndef SSND_H
#define SSND_H

#include <stdbool.h>
#include <stdint.h>

typedef struct SsndHandle SsndHandle;

// mode: "game", "balanced" or "music". announce: make this device visible
// to other devices (the app does, the broadcast extension doesn't).
SsndHandle *ssnd_create(const char *name, const char *mode, bool announce);
void ssnd_destroy(SsndHandle *h);
void ssnd_free_string(char *s);

char *ssnd_start_receiving(const SsndHandle *h);
void ssnd_stop_receiving(const SsndHandle *h);
// Audio render thread: mixed received audio into two channel buffers
// (right may be NULL for mono). Writes silence when nothing is playing.
void ssnd_render_planar(const SsndHandle *h, float *left, float *right, uint32_t frames, uint32_t rate);

void ssnd_set_output_latency(float ms);
void ssnd_set_capture_latency(float ms);

// dests: comma-separated "ip" or "ip:port".
char *ssnd_start_sending(const SsndHandle *h, const char *dests);
void ssnd_stop_sending(const SsndHandle *h);
void ssnd_push_capture(const SsndHandle *h, const float *data, uint32_t len, uint32_t rate, uint32_t ch);
void ssnd_push_capture_planar(const SsndHandle *h, const float *left, const float *right, uint32_t frames, uint32_t rate);
void ssnd_set_send_dests(const SsndHandle *h, const char *dests);

void ssnd_set_forward(const SsndHandle *h, const char *dests);
void ssnd_set_play_local(const SsndHandle *h, bool on);
// 0.0 ... 1.5
void ssnd_set_volume(const SsndHandle *h, float v);
// Volume for one sender, by its IP (StreamStats "from"), 0..2, 1 = as sent.
// Multiplies on top of ssnd_set_volume; can be set before it starts sending.
void ssnd_set_source_volume(const SsndHandle *h, const char *from, float v);
void ssnd_set_mode(const SsndHandle *h, const char *mode);
void ssnd_set_name(const SsndHandle *h, const char *name);
void ssnd_set_manual_peers(const SsndHandle *h, const char *ips);

// tap: 0 send, 1 receive. kind: 0 spectrum (n bands), 1 waveform (n columns).
// Writes n values in 0...1; returns false while that side is off.
bool ssnd_scope(const SsndHandle *h, int32_t tap, int32_t kind, uint32_t n, float *out);

char *ssnd_state_json(const SsndHandle *h);

// App <-> broadcast extension over loopback UDP (127.0.0.1).
typedef struct SsndLink SsndLink;
SsndLink *ssnd_link_open(uint16_t port);
void ssnd_link_close(SsndLink *l);
// Newest waiting message into buf; its length, or -1 when none.
int32_t ssnd_link_recv(const SsndLink *l, uint8_t *buf, uint32_t cap);
bool ssnd_link_send(uint16_t port, const uint8_t *data, uint32_t len);
// Extension: send a JSON report on sending (with visualizer data) to port.
bool ssnd_link_report(const SsndHandle *h, uint16_t port, const char *note);

#endif
