#ifndef PLIANT_EMBEDDER_TEST_HOST_H_
#define PLIANT_EMBEDDER_TEST_HOST_H_

#include <stdint.h>

extern "C" {

typedef void (*PliantEmbedderTestCallback)(void* context, uint32_t action);

void pliant_embedder_test_host_set_diagnostic_log(const char* path);
const char* pliant_embedder_test_host_last_error();
void* pliant_embedder_test_host_create(PliantEmbedderTestCallback callback,
                                       void* context,
                                       const char* url_a,
                                       const char* url_b);
void pliant_embedder_test_host_destroy(void* host);
uint8_t pliant_embedder_test_host_show(void* host);
uint8_t pliant_embedder_test_host_set_status(void* host,
                                             const char* text,
                                             uint8_t is_error);
uint8_t pliant_embedder_test_host_set_identity(void* host, const char* text);
void* pliant_embedder_test_host_content_container(void* host);

}

#endif  // PLIANT_EMBEDDER_TEST_HOST_H_
