#ifndef PLIANT_BROWSER_HOST_H_
#define PLIANT_BROWSER_HOST_H_

#include <stdint.h>

extern "C" {

typedef void (*PliantBrowserHostCallback)(void* context,
                                         uint32_t kind,
                                         uint64_t generation,
                                         uint64_t page,
                                         const char* node_id,
                                         const char* value);

void* pliant_browser_host_create(PliantBrowserHostCallback callback,
                                 void* context);
void pliant_browser_host_destroy(void* host);
void pliant_browser_host_show(void* host);
void pliant_browser_host_set_preview_mode(void* host, uint8_t preview_mode);
void pliant_browser_host_set_preview_pending(void* host, uint8_t pending);
void pliant_browser_host_set_preview_checking(void* host, uint8_t checking);
void pliant_browser_host_set_save_pending(void* host, uint8_t pending);
void pliant_browser_host_set_definition_path(void* host, const char* path);
void pliant_browser_host_set_status(void* host,
                                    const char* text,
                                    uint8_t is_error);

int pliant_browser_host_begin_layout(void* host, uint64_t generation);
int pliant_browser_host_add_container(void* host,
                                      const char* parent,
                                      const char* node_id,
                                      uint8_t axis,
                                      double gap);
int pliant_browser_host_add_spacer(void* host,
                                   const char* parent,
                                   const char* node_id,
                                   double size);
int pliant_browser_host_add_label(void* host,
                                  const char* parent,
                                  const char* node_id,
                                  const char* text);
int pliant_browser_host_add_address(void* host,
                                    const char* parent,
                                    const char* node_id,
                                    const char* placeholder);
int pliant_browser_host_add_page_list(void* host,
                                      const char* parent,
                                      const char* node_id);
int pliant_browser_host_add_content(void* host,
                                    const char* parent,
                                    const char* node_id);
int pliant_browser_host_add_button(void* host,
                                   const char* parent,
                                   const char* node_id,
                                   const char* label);
int pliant_browser_host_end_layout(void* host);

void pliant_browser_host_pages_begin(void* host);
void pliant_browser_host_page_add(void* host,
                                  uint64_t page,
                                  const char* label,
                                  uint8_t selected);
void pliant_browser_host_pages_end(void* host);
void pliant_browser_host_set_current_url(void* host, const char* url);
void pliant_browser_host_set_node_enabled(void* host,
                                         const char* node_id,
                                         uint8_t enabled);
void* pliant_browser_host_content_container(void* host);

}

#endif  // PLIANT_BROWSER_HOST_H_
