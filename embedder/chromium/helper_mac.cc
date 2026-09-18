// Keep the helper minimal: engage Chromium's seatbelt sandbox before loading
// the framework or running any Rust/application callback.
#include "pliant_mvp/public/c/bridge.h"

#include <dlfcn.h>
#include <mach-o/dyld.h>

#include <filesystem>
#include <iostream>
#include <string>

#include "base/allocator/early_zone_registration_apple.h"
#include "sandbox/mac/seatbelt_exec.h"

int main(int argc, char** argv) {
  partition_alloc::EarlyMallocZoneRegistration();
  uint32_t size = 0;
  _NSGetExecutablePath(nullptr, &size);
  std::string executable(size, '\0');
  if (_NSGetExecutablePath(executable.data(), &size) != 0) return 1;
  executable.resize(executable.find('\0'));

  auto sandbox = sandbox::SeatbeltExecServer::CreateFromArguments(
      executable.c_str(), argc, argv);
  if (sandbox.sandbox_required &&
      (!sandbox.server || !sandbox.server->InitializeSandbox())) {
    std::cerr << "Pliant helper could not initialize the sandbox.\n";
    return 1;
  }
  auto framework = std::filesystem::path(executable).parent_path() /
                   "../../../../PliantContent";
  void* library = dlopen(framework.c_str(), RTLD_NOW | RTLD_LOCAL | RTLD_FIRST);
  if (!library) {
    std::cerr << "Cannot load PliantContent: " << dlerror() << '\n';
    return 1;
  }
  using Entry = decltype(&pliant_engine_run);
  auto entry = reinterpret_cast<Entry>(dlsym(library, "pliant_engine_run"));
  if (!entry) {
    std::cerr << "Cannot find the Pliant entry point: " << dlerror() << '\n';
    return 1;
  }
  return entry(argc, argv, nullptr, nullptr);
}