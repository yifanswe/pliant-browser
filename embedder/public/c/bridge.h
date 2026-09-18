#ifndef PLIANT_MVP_BRIDGE_H_
#define PLIANT_MVP_BRIDGE_H_

#include <stddef.h>
#include <stdint.h>

#define PLIANT_EXPORT __attribute__((visibility("default")))

extern "C" {

struct PliantEvent {
  // 0 ready, 1 navigated, 2 navigation failed, 3 closed,
  // 4 renderer failed, 5 painted, 6 close refused.
  uint32_t kind;
  uint64_t page;
  int32_t code;
  const char* url;
  size_t url_len;
  uint8_t can_go_back;
  uint8_t can_go_forward;
};

using PliantCallback = void (*)(void*, const PliantEvent*);

PLIANT_EXPORT int pliant_engine_run(int argc,
                                   const char* const* argv,
                                   PliantCallback callback,
                                   void* user_data);
PLIANT_EXPORT int pliant_page_create(const char* url, uint64_t* page);
// The trusted host owns `container` (an NSView*) and keeps it alive until the
// page is detached or closed.
PLIANT_EXPORT int pliant_page_create_in(const char* url,
                                       void* container,
                                       uint64_t* page);
PLIANT_EXPORT int pliant_page_load(uint64_t page, const char* url);
PLIANT_EXPORT int pliant_page_back(uint64_t page);
PLIANT_EXPORT int pliant_page_forward(uint64_t page);
PLIANT_EXPORT int pliant_page_attach(uint64_t page, void* container);
PLIANT_EXPORT int pliant_page_detach(uint64_t page);
PLIANT_EXPORT int pliant_page_close(uint64_t page);
PLIANT_EXPORT int pliant_engine_check_ready();
PLIANT_EXPORT int pliant_engine_shutdown();

}

#endif  // PLIANT_MVP_BRIDGE_H_
