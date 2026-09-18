#include "pliant_mvp/public/c/bridge.h"

#import <AppKit/AppKit.h>
#include <pthread.h>

#include <map>
#include <memory>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "base/apple/bundle_locations.h"
#include "base/apple/foundation_util.h"
#include "base/auto_reset.h"
#include "base/check.h"
#include "base/command_line.h"
#include "base/compiler_specific.h"
#include "base/containers/span.h"
#include "base/files/file_path.h"
#include "base/files/scoped_temp_dir.h"
#include "base/functional/bind.h"
#include "base/location.h"
#include "base/mac/scoped_sending_event.h"
#include "base/memory/raw_ptr.h"
#include "base/memory/weak_ptr.h"
#include "base/message_loop/message_pump_apple.h"
#include "base/observer_list.h"
#include "base/path_service.h"
#include "base/run_loop.h"
#include "base/strings/sys_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "content/public/app/content_main.h"
#include "content/public/app/content_main_delegate.h"
#include "content/public/browser/browser_context.h"
#include "content/public/browser/browser_main_parts.h"
#include "content/public/browser/browser_thread.h"
#include "content/public/browser/content_browser_client.h"
#include "content/public/browser/download_manager_delegate.h"
#include "content/public/browser/native_event_processor_mac.h"
#include "content/public/browser/native_event_processor_observer_mac.h"
#include "content/public/browser/navigation_controller.h"
#include "content/public/browser/navigation_handle.h"
#include "content/public/browser/web_contents.h"
#include "content/public/browser/web_contents_delegate.h"
#include "content/public/browser/web_contents_observer.h"
#include "content/public/browser/zoom_level_delegate.h"
#include "content/public/common/content_client.h"
#include "content/public/common/content_paths.h"
#include "content/public/common/content_switches.h"
#include "services/network/public/mojom/network_context.mojom.h"
#include "ui/base/resource/resource_bundle.h"
#include "ui/display/screen.h"
#include "url/gurl.h"

// Chromium's AppKit event loop expects these protocols, not just NSApplication.
@interface PliantApplication
    : NSApplication <CrAppProtocol, CrAppControlProtocol, NativeEventProcessor>
@end

@implementation PliantApplication {
  BOOL _handlingSendEvent;
  base::ObserverList<content::NativeEventProcessorObserver>::Unchecked
      _eventObservers;
}
- (BOOL)isHandlingSendEvent { return _handlingSendEvent; }
- (void)setHandlingSendEvent:(BOOL)value { _handlingSendEvent = value; }
- (void)sendEvent:(NSEvent*)event {
  base::AutoReset<BOOL> scope(&_handlingSendEvent, YES);
  content::ScopedNotifyNativeEventProcessorObserver notify(&_eventObservers,
                                                         event);
  [super sendEvent:event];
}
- (void)addNativeEventProcessorObserver:
    (content::NativeEventProcessorObserver*)observer {
  _eventObservers.AddObserver(observer);
}
- (void)removeNativeEventProcessorObserver:
    (content::NativeEventProcessorObserver*)observer {
  _eventObservers.RemoveObserver(observer);
}
- (void)terminate:(id)sender {
  pliant_engine_shutdown();
}
@end

@interface PliantWindowDelegate : NSObject <NSWindowDelegate>
@property(nonatomic) uint64_t page;
@end

@implementation PliantWindowDelegate
@synthesize page = _page;

- (BOOL)windowShouldClose:(NSWindow*)sender {
  // Engine teardown closes the NSWindow after its content instance is released.
  pliant_page_close(self.page);
  return NO;
}
@end

namespace {

enum Status {
  kOk = 0,
  kNotReady = 1,
  kWrongThread = 2,
  kInvalidPage = 3,
  kInvalidUrl = 4,
  kNoHistoryEntry = 5,
  kCloseNeedsConfirmation = 6,
  kCreationFailed = 7,
};

enum EventKind : uint32_t {
  kReady = 0,
  kNavigated = 1,
  kNavigationFailed = 2,
  kClosed = 3,
  kRendererFailed = 4,
  kPainted = 5,
  kCloseRefused = 6,
};

class DenyDownloads final : public content::DownloadManagerDelegate {
 public:
  ~DenyDownloads() override = default;
  bool DetermineDownloadTarget(download::DownloadItem*,
                               download::DownloadTargetCallback* callback) override {
    // An empty destination cancels, rather than using an implicit default path.
    std::move(*callback).Run(download::DownloadTargetInfo());
    return true;
  }
};

// One disposable context. No Chrome profile services or permissive test defaults.
class Context final : public content::BrowserContext {
 public:
  Context(base::FilePath directory, DenyDownloads* downloads)
      : directory_(std::move(directory)), downloads_(downloads) {}
  ~Context() override {
    NotifyWillBeDestroyed();
    ShutdownStoragePartitions();
  }
  base::FilePath GetPath() const override { return directory_; }
  bool IsOffTheRecord() override { return true; }
  std::unique_ptr<content::ZoomLevelDelegate> CreateZoomLevelDelegate(
      const base::FilePath&) override { return nullptr; }
  content::DownloadManagerDelegate* GetDownloadManagerDelegate() override {
    return downloads_;
  }
  content::BrowserPluginGuestManager* GetGuestManager() override { return nullptr; }
  storage::SpecialStoragePolicy* GetSpecialStoragePolicy() override { return nullptr; }
  content::PlatformNotificationService* GetPlatformNotificationService() override {
    return nullptr;
  }
  content::PushMessagingService* GetPushMessagingService() override { return nullptr; }
  content::StorageNotificationService* GetStorageNotificationService() override {
    return nullptr;
  }
  content::SSLHostStateDelegate* GetSSLHostStateDelegate() override { return nullptr; }
  content::PermissionControllerDelegate* GetPermissionControllerDelegate() override {
    // In the pinned Content implementation, an absent delegate denies requests
    // and status queries. There is no DevTools override or permission bypass.
    return nullptr;
  }
  content::BackgroundFetchDelegate* GetBackgroundFetchDelegate() override {
    return nullptr;
  }
  content::BackgroundSyncController* GetBackgroundSyncController() override {
    return nullptr;
  }
  content::BrowsingDataRemoverDelegate* GetBrowsingDataRemoverDelegate() override {
    return nullptr;
  }
  content::ClientHintsControllerDelegate* GetClientHintsControllerDelegate() override {
    return nullptr;
  }
  content::ReduceAcceptLanguageControllerDelegate*
  GetReduceAcceptLanguageControllerDelegate() override { return nullptr; }

 private:
  const base::FilePath directory_;
  // Runtime owns this delegate and keeps it alive through ~BrowserContext.
  raw_ptr<DenyDownloads> downloads_;
};

class Runtime;

class Page final : public content::WebContentsObserver,
                   public content::WebContentsDelegate {
 public:
  Page(Runtime* runtime, uint64_t id, content::BrowserContext* context,
       NSView* container);
  ~Page() override;
  void Attach(NSView* container);
  void Detach();
  void RequestClose();
  void DidFinishNavigation(content::NavigationHandle* navigation) override;
  void DidFirstVisuallyNonEmptyPaint() override;
  void PrimaryMainFrameRenderProcessGone(base::TerminationStatus) override;
  using content::WebContentsObserver::BeforeUnloadFired;
  void BeforeUnloadFired(content::WebContents*,
                         bool proceed,
                         bool* proceed_to_fire_unload) override;
  void CloseContents(content::WebContents*) override;
  void CanDownload(const GURL&, const std::string&,
                   base::OnceCallback<void(bool)> callback) override {
    std::move(callback).Run(false);
  }
  bool CanEnterFullscreenModeForTab(content::RenderFrameHost*) override {
    return false;
  }
  bool CanDragEnter(content::WebContents*, const content::DropData&,
                    blink::DragOperationsMask) override { return false; }
  bool closing = false;

 private:
  const raw_ptr<Runtime> runtime_;
  const uint64_t id_;
  NSView* __weak container_;
  NSWindow* __strong window_;
  PliantWindowDelegate* __strong window_delegate_;
  std::unique_ptr<content::WebContents> contents_;
  bool close_requested_ = false;
};

class Runtime {
 public:
  Runtime(PliantCallback callback, void* data, base::FilePath directory)
      : callback_(callback), data_(data), directory_(std::move(directory)) {}
  const base::FilePath& directory() const { return directory_; }

  void Initialize() {
    context_ = std::make_unique<Context>(directory_, &downloads_);
  }
  void Ready(base::RepeatingClosure quit) {
    quit_ = std::move(quit);
    Queue(kReady, 0);
  }
  int CheckReady() const {
    if (!content::BrowserThread::CurrentlyOn(content::BrowserThread::UI)) {
      return kWrongThread;
    }
    return context_ && !stopping_ ? kOk : kNotReady;
  }
  Page* Find(uint64_t id) const {
    const auto found = pages_.find(id);
    return found == pages_.end() || found->second->closing
               ? nullptr : found->second.get();
  }
  int Create(const GURL& url, NSView* container, uint64_t* id) {
    const uint64_t value = next_id_++;
    pages_.emplace(value,
                   std::make_unique<Page>(this, value, context_.get(), container));
    *id = value;
    return Load(value, url);
  }
  int Load(uint64_t id, const GURL& url) {
    auto* page = Find(id);
    if (!page) return kInvalidPage;
    page->web_contents()->GetController().LoadURLWithParams(
        content::NavigationController::LoadURLParams(url));
    return kOk;
  }
  int History(uint64_t id, bool forward) {
    auto* page = Find(id);
    if (!page) return kInvalidPage;
    auto& controller = page->web_contents()->GetController();
    if (forward ? !controller.CanGoForward() : !controller.CanGoBack()) {
      return kNoHistoryEntry;
    }
    if (forward) controller.GoForward();
    else controller.GoBack();
    return kOk;
  }
  int Attach(uint64_t id, NSView* container) {
    auto* page = Find(id);
    if (!page) return kInvalidPage;
    if (!container) return kCreationFailed;
    page->Attach(container);
    return kOk;
  }
  int Detach(uint64_t id) {
    auto* page = Find(id);
    if (!page) return kInvalidPage;
    page->Detach();
    return kOk;
  }
  int Close(uint64_t id) {
    auto* page = Find(id);
    if (!page) return kInvalidPage;
    page->RequestClose();
    return kOk;
  }
  void RemoveLater(uint64_t id) {
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostNonNestableTask(
        FROM_HERE, base::BindOnce(&Runtime::Remove, weak_.GetWeakPtr(), id));
  }
  void Queue(uint32_t kind, uint64_t id, int code = 0, std::string url = {},
             bool back = false, bool forward = false) {
    if (stopping_) return;
    // Rust lends one mutable handler. A nested AppKit loop must not reenter it.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostNonNestableTask(
        FROM_HERE, base::BindOnce(&Runtime::Deliver, weak_.GetWeakPtr(), kind,
                                 id, code, std::move(url), back, forward));
  }
  void RequestShutdown() {
    if (stopping_) return;
    stopping_ = true;
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostNonNestableTask(
        FROM_HERE, base::BindOnce(&Runtime::Shutdown, weak_.GetWeakPtr()));
  }
  void Finish() {
    weak_.InvalidateWeakPtrs();
    pages_.clear();
    context_.reset();
  }

 private:
  void Remove(uint64_t id) {
    if (pages_.erase(id)) Queue(kClosed, id);
  }
  void Deliver(uint32_t kind, uint64_t id, int code, std::string url,
               bool back, bool forward) {
    if (stopping_) return;
    const PliantEvent event{kind, id, code, url.data(), url.size(),
                           static_cast<uint8_t>(back),
                           static_cast<uint8_t>(forward)};
    callback_(data_, &event);
  }
  void Shutdown() {
    Finish();
    quit_.Run();
  }
  const PliantCallback callback_;
  const raw_ptr<void> data_;
  const base::FilePath directory_;
  DenyDownloads downloads_;
  std::unique_ptr<Context> context_;
  std::map<uint64_t, std::unique_ptr<Page>> pages_;
  base::RepeatingClosure quit_;
  uint64_t next_id_ = 1;
  bool stopping_ = false;
  base::WeakPtrFactory<Runtime> weak_{this};
};

Page::Page(Runtime* runtime, uint64_t id, content::BrowserContext* context,
           NSView* container)
    : runtime_(runtime), id_(id) {
  content::WebContents::CreateParams params(context);
  params.initially_hidden = true;
  contents_ = content::WebContents::Create(params);
  CHECK(contents_);
  Observe(contents_.get());
  contents_->SetDelegate(this);

  if (!container) {
    // Preserve the original independent test-window API.
    window_ = [[NSWindow alloc]
        initWithContentRect:NSMakeRect(0, 0, 960, 640)
                  styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
                            NSWindowStyleMaskMiniaturizable | NSWindowStyleMaskResizable
                    backing:NSBackingStoreBuffered
                      defer:NO];
    window_.releasedWhenClosed = NO;
    window_.title = @"Pliant MVP";
    window_delegate_ = [[PliantWindowDelegate alloc] init];
    window_delegate_.page = id;
    window_.delegate = window_delegate_;
    container = window_.contentView;
  }
  Attach(container);
  if (window_) {
    [window_ center];
    [window_ makeKeyAndOrderFront:nil];
  }
}

void Page::Attach(NSView* container) {
  Detach();
  NSView* view = contents_->GetNativeView().GetNativeNSView();
  container_ = container;
  view.frame = container.bounds;
  view.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
  [container addSubview:view];
  contents_->WasShown();
  contents_->Focus();
}

void Page::Detach() {
  if (!container_) return;
  contents_->WasHidden();
  [contents_->GetNativeView().GetNativeNSView() removeFromSuperview];
  container_ = nil;
}

void Page::RequestClose() {
  if (close_requested_) return;
  close_requested_ = true;
  // Auto-cancel requests that need a dialog. BeforeUnloadFired preserves
  // Chromium's result; its close manager continues with ClosePage on success.
  web_contents()->DispatchBeforeUnload(/*auto_cancel=*/true);
}

Page::~Page() {
  Observe(nullptr);
  contents_->SetDelegate(nullptr);
  Detach();
  contents_.reset();
  window_.delegate = nil;
  [window_ close];
}

void Page::DidFinishNavigation(content::NavigationHandle* navigation) {
  if (!navigation->IsInPrimaryMainFrame() || closing) return;
  if (navigation->IsErrorPage() || navigation->GetNetErrorCode() != 0) {
    runtime_->Queue(kNavigationFailed, id_, navigation->GetNetErrorCode());
  } else if (navigation->HasCommitted()) {
    auto& controller = contents_->GetController();
    runtime_->Queue(kNavigated, id_, 0, navigation->GetURL().spec(),
                    controller.CanGoBack(), controller.CanGoForward());
  }
}

void Page::DidFirstVisuallyNonEmptyPaint() {
  if (!closing && container_ && container_.window.visible) {
    runtime_->Queue(kPainted, id_, 0, contents_->GetLastCommittedURL().spec());
  }
}

void Page::PrimaryMainFrameRenderProcessGone(base::TerminationStatus) {
  runtime_->Queue(kRendererFailed, id_);
}

void Page::BeforeUnloadFired(content::WebContents*,
                             bool proceed,
                             bool* proceed_to_fire_unload) {
  *proceed_to_fire_unload = proceed;
  if (!proceed && close_requested_) {
    close_requested_ = false;
    runtime_->Queue(kCloseRefused, id_);
  }
}

void Page::CloseContents(content::WebContents*) {
  closing = true;
  runtime_->RemoveLater(id_);
}

class MainParts final : public content::BrowserMainParts {
 public:
  explicit MainParts(Runtime* host) : host_(host) {}
  int PreMainMessageLoopRun() override {
    screen_ = std::make_unique<display::ScopedNativeScreen>();
    host_->Initialize();
    return 0;
  }
  void WillRunMainMessageLoop(std::unique_ptr<base::RunLoop>& loop) override {
    host_->Ready(loop->QuitClosure());
  }
  void PostMainMessageLoopRun() override { host_->Finish(); }
 private:
  const raw_ptr<Runtime> host_;
  std::unique_ptr<display::ScopedNativeScreen> screen_;
};

class BrowserClient final : public content::ContentBrowserClient {
 public:
  explicit BrowserClient(Runtime* host) : host_(host) {}
  std::unique_ptr<content::BrowserMainParts> CreateBrowserMainParts(bool) override {
    return std::make_unique<MainParts>(host_);
  }
  bool CanCreateWindow(content::RenderFrameHost*, const GURL&, const GURL&,
                       const url::Origin&, content::mojom::WindowContainerType,
                       const GURL&, const content::Referrer&, const std::string&,
                       WindowOpenDisposition, const blink::mojom::WindowFeatures&,
                       bool, bool, bool*) override { return false; }
  bool IsFileAccessAllowed(const base::FilePath&, const base::FilePath&,
                            const base::FilePath&) override { return false; }
  std::string GetUserAgent() override { return "PliantMvp/0.1"; }
  void ConfigureNetworkContextParams(
      content::BrowserContext*, bool, const base::FilePath&,
      network::mojom::NetworkContextParams* params,
      cert_verifier::mojom::CertVerifierCreationParams*) override {
    params->user_agent = GetUserAgent();
    params->accept_language = "en-US,en";
  }
  std::vector<base::FilePath> GetNetworkContextsParentDirectory() override {
    return {host_->directory()};
  }
  base::FilePath GetSandboxedStorageServiceDataDirectory() override {
    return host_->directory();
  }
 private:
  const raw_ptr<Runtime> host_;
};

class ContentClient final : public content::ContentClient {
 public:
  std::u16string GetLocalizedString(int id) override {
    return ui::ResourceBundle::GetSharedInstance().GetLocalizedString(id);
  }
  std::string_view GetDataResource(int id, ui::ResourceScaleFactor scale) override {
    return ui::ResourceBundle::GetSharedInstance().GetRawDataResourceForScale(id, scale);
  }
  base::RefCountedMemory* GetDataResourceBytes(int id) override {
    return ui::ResourceBundle::GetSharedInstance().LoadDataResourceBytes(id);
  }
  std::string GetDataResourceString(int id) override {
    return ui::ResourceBundle::GetSharedInstance().LoadDataResourceString(id);
  }
  gfx::Image& GetNativeImageNamed(int id) override {
    return ui::ResourceBundle::GetSharedInstance().GetNativeImageNamed(id);
  }
};

class MainDelegate final : public content::ContentMainDelegate {
 public:
  explicit MainDelegate(Runtime* host) : browser_(host) {}
  void PreSandboxStartup() override {
    const auto pak = base::apple::FrameworkBundlePath().Append("Resources/pliant.pak");
    ui::ResourceBundle::InitSharedInstanceWithPakPath(pak);
  }
  std::optional<int> PreBrowserMain() override {
    [PliantApplication sharedApplication];
    CHECK([NSApp isKindOfClass:[PliantApplication class]]);
    NSApp.activationPolicy = NSApplicationActivationPolicyRegular;
    return std::nullopt;
  }
  content::ContentClient* CreateContentClient() override { return &content_; }
  content::ContentBrowserClient* CreateContentBrowserClient() override {
    return &browser_;
  }
 private:
  ContentClient content_;
  BrowserClient browser_;
};

Runtime* runtime = nullptr;
int Ready() {
  // The runtime pointer is owned by the main thread, including startup/teardown.
  if (!pthread_main_np()) return kWrongThread;
  return runtime ? runtime->CheckReady() : kNotReady;
}
bool Allowed(const GURL& url) {
  return url.is_valid() && (url.SchemeIsHTTPOrHTTPS() || url == GURL("about:blank"));
}

void SetBundlePaths() {
  NSBundle* framework = [NSBundle bundleForClass:[PliantApplication class]];
  CHECK(framework);
  base::apple::SetOverrideFrameworkBundle(framework);
  base::FilePath root(base::SysNSStringToUTF8(framework.bundlePath));
  // NSBundle may resolve the versioned directory rather than the framework root.
  while (root.Extension() != ".framework" && root != root.DirName()) {
    root = root.DirName();
  }
  CHECK(root.Extension() == ".framework");
  base::apple::SetOverrideOuterBundlePath(root.DirName().DirName().DirName());
  base::apple::SetBaseBundleIDOverride("org.pliant.mvp");
  CHECK(base::PathService::OverrideAndCreateIfNeeded(
      content::CHILD_PROCESS_EXE,
      root.Append("Helpers/PliantHelper.app/Contents/MacOS/PliantHelper"),
      /*is_absolute=*/true, /*create=*/false));
}

}  // namespace

extern "C" int pliant_engine_run(int argc, const char* const* argv,
                                  PliantCallback callback, void* data) {
  if (!pthread_main_np()) return kWrongThread;
  if (argc < 1 || !argv || !argv[0]) return kNotReady;
  static bool started = false;
  if (started) return kNotReady;
  started = true;
  base::CommandLine::Init(argc, argv);
  const auto& command = *base::CommandLine::ForCurrentProcess();
  const bool child = command.HasSwitch(switches::kProcessType);
  if (!child && !callback) return kNotReady;
  for (const char* prohibited : {"no-sandbox", "disable-web-security",
                                "ignore-certificate-errors", "single-process",
                                "remote-debugging-port", "remote-debugging-pipe",
                                "browser-subprocess-path"}) {
    if (command.HasSwitch(prohibited)) return kCreationFailed;
  }
  SetBundlePaths();
  base::ScopedTempDir directory;
  if (!child && !directory.CreateUniqueTempDir()) return kCreationFailed;
  Runtime host(callback, data, child ? base::FilePath() : directory.GetPath());
  MainDelegate delegate(child ? nullptr : &host);
  content::ContentMainParams params(&delegate);
  auto arguments = UNSAFE_BUFFERS(base::span(argv, static_cast<size_t>(argc)));
  std::vector<const char*> pointers(arguments.begin(), arguments.end());
  pointers.push_back(nullptr);  // Preserve the native argv[argc] sentinel.
  params.argc = argc;
  params.argv = pointers.data();
  if (!child) runtime = &host;
  const int result = content::ContentMain(std::move(params));
  host.Finish();
  runtime = nullptr;
  return result;
}

extern "C" int pliant_page_create(const char* text, uint64_t* page) {
  if (const int result = Ready(); result != kOk) return result;
  if (!text || !page) return kInvalidUrl;
  const GURL url(text);
  return Allowed(url) ? runtime->Create(url, nullptr, page) : kInvalidUrl;
}
extern "C" int pliant_page_create_in(const char* text, void* container,
                                      uint64_t* page) {
  if (const int result = Ready(); result != kOk) return result;
  if (!text || !container || !page) return kInvalidUrl;
  const GURL url(text);
  return Allowed(url)
             ? runtime->Create(url, (__bridge NSView*)container, page)
             : kInvalidUrl;
}
extern "C" int pliant_page_load(uint64_t page, const char* text) {
  if (const int result = Ready(); result != kOk) return result;
  if (!text) return kInvalidUrl;
  const GURL url(text);
  return Allowed(url) ? runtime->Load(page, url) : kInvalidUrl;
}
extern "C" int pliant_page_back(uint64_t page) {
  const int result = Ready();
  return result == kOk ? runtime->History(page, false) : result;
}
extern "C" int pliant_page_forward(uint64_t page) {
  const int result = Ready();
  return result == kOk ? runtime->History(page, true) : result;
}
extern "C" int pliant_page_attach(uint64_t page, void* container) {
  const int result = Ready();
  return result == kOk
             ? runtime->Attach(page, (__bridge NSView*)container)
             : result;
}
extern "C" int pliant_page_detach(uint64_t page) {
  const int result = Ready();
  return result == kOk ? runtime->Detach(page) : result;
}
extern "C" int pliant_page_close(uint64_t page) {
  const int result = Ready();
  return result == kOk ? runtime->Close(page) : result;
}
extern "C" int pliant_engine_check_ready() {
  return Ready();
}
extern "C" int pliant_engine_shutdown() {
  const int result = Ready();
  if (result == kOk) runtime->RequestShutdown();
  return result;
}