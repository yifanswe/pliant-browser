#include "host.h"

#import <AppKit/AppKit.h>

#include <fcntl.h>
#include <unistd.h>

#include <string>

namespace {

std::string diagnostic_path;
std::string last_error;

NSString* Text(const char* value) {
  if (!value) return @"";
  return [NSString stringWithUTF8String:value] ?: @"";
}

void Log(const std::string& message) {
  if (diagnostic_path.empty()) return;
  const int descriptor =
      open(diagnostic_path.c_str(), O_WRONLY | O_APPEND | O_CLOEXEC);
  if (descriptor < 0) return;
  const std::string line = "native: " + message + "\n";
  const ssize_t ignored = write(descriptor, line.data(), line.size());
  (void)ignored;
  close(descriptor);
}

void ClearError() {
  last_error.clear();
}

void RecordException(const char* operation, NSException* exception) {
  NSString* reason = exception.reason ?: @"no reason";
  NSString* name = exception.name ?: @"NSException";
  last_error = std::string(operation) + " raised " + name.UTF8String + ": " +
               reason.UTF8String;
  Log(last_error);
}

enum TestAction : uint32_t {
  kCreateA = 0,
  kLoadA = 1,
  kLoadB = 2,
  kBack = 3,
  kForward = 4,
  kClose = 5,
  kProbeStale = 6,
  kQuit = 7,
};

}  // namespace

@interface PliantEmbedderTestHost : NSObject <NSWindowDelegate>
@property(nonatomic) PliantEmbedderTestCallback callback;
@property(nonatomic) void* callbackContext;
@property(nonatomic, strong) NSWindow* window;
@property(nonatomic, strong) NSTextField* statusLabel;
@property(nonatomic, strong) NSTextField* identityLabel;
@property(nonatomic, strong) NSView* contentContainer;
- (instancetype)initWithCallback:(PliantEmbedderTestCallback)callback
                         context:(void*)context
                            urlA:(NSString*)urlA
                            urlB:(NSString*)urlB;
@end

@implementation PliantEmbedderTestHost

- (instancetype)initWithCallback:(PliantEmbedderTestCallback)callback
                         context:(void*)context
                            urlA:(NSString*)urlA
                            urlB:(NSString*)urlB {
  self = [super init];
  if (!self) return nil;
  _callback = callback;
  _callbackContext = context;
  _window = [[NSWindow alloc]
      initWithContentRect:NSMakeRect(0, 0, 1120, 760)
                styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
                          NSWindowStyleMaskMiniaturizable |
                          NSWindowStyleMaskResizable
                  backing:NSBackingStoreBuffered
                    defer:NO];
  _window.releasedWhenClosed = NO;
  _window.title = @"Pliant Embedder Manual API Acceptance";
  _window.delegate = self;

  NSStackView* root = [NSStackView stackViewWithViews:@[]];
  root.orientation = NSUserInterfaceLayoutOrientationVertical;
  root.alignment = NSLayoutAttributeLeading;
  root.spacing = 8;
  root.edgeInsets = NSEdgeInsetsMake(10, 10, 10, 10);
  root.translatesAutoresizingMaskIntoConstraints = NO;
  [_window.contentView addSubview:root];
  [NSLayoutConstraint activateConstraints:@[
    [root.leadingAnchor constraintEqualToAnchor:_window.contentView.leadingAnchor],
    [root.trailingAnchor constraintEqualToAnchor:_window.contentView.trailingAnchor],
    [root.topAnchor constraintEqualToAnchor:_window.contentView.topAnchor],
    [root.bottomAnchor constraintEqualToAnchor:_window.contentView.bottomAnchor],
  ]];

  NSTextField* title = [NSTextField
      labelWithString:@"Public pliant-embedder API controls (manual; loopback only)"];
  title.font = [NSFont boldSystemFontOfSize:14];
  [root addArrangedSubview:title];

  NSStackView* controls = [NSStackView stackViewWithViews:@[]];
  controls.orientation = NSUserInterfaceLayoutOrientationHorizontal;
  controls.alignment = NSLayoutAttributeCenterY;
  controls.spacing = 6;
  for (NSArray<id>* spec in @[
         @[@"Create A", @(kCreateA)],
         @[@"Load A", @(kLoadA)],
         @[@"Load B", @(kLoadB)],
         @[@"Back", @(kBack)],
         @[@"Forward", @(kForward)],
         @[@"Close", @(kClose)],
         @[@"Probe stale ID", @(kProbeStale)],
       ]) {
    NSButton* button = [NSButton buttonWithTitle:spec[0]
                                         target:self
                                         action:@selector(runAction:)];
    button.tag = [spec[1] integerValue];
    [controls addArrangedSubview:button];
  }
  [root addArrangedSubview:controls];

  NSTextField* fixtureLabel = [NSTextField wrappingLabelWithString:
      [NSString stringWithFormat:@"Fixture A: %@\nFixture B: %@", urlA, urlB]];
  fixtureLabel.selectable = YES;
  [root addArrangedSubview:fixtureLabel];

  _identityLabel =
      [NSTextField wrappingLabelWithString:@"Active page: none | stale page: none"];
  _identityLabel.selectable = YES;
  [root addArrangedSubview:_identityLabel];

  _statusLabel = [NSTextField
      wrappingLabelWithString:@"Ready. Click Create A to create the first page."];
  _statusLabel.selectable = YES;
  [root addArrangedSubview:_statusLabel];

  _contentContainer = [[NSView alloc] initWithFrame:NSZeroRect];
  _contentContainer.translatesAutoresizingMaskIntoConstraints = NO;
  _contentContainer.wantsLayer = YES;
  _contentContainer.layer.borderWidth = 1;
  _contentContainer.layer.borderColor = NSColor.separatorColor.CGColor;
  [root addArrangedSubview:_contentContainer];
  [_contentContainer.heightAnchor constraintGreaterThanOrEqualToConstant:560].active =
      YES;
  [_contentContainer.widthAnchor constraintEqualToAnchor:root.widthAnchor].active =
      YES;
  return self;
}

- (void)runAction:(NSButton*)sender {
  if (self.callback) {
    self.callback(self.callbackContext, static_cast<uint32_t>(sender.tag));
  }
}

- (void)windowWillClose:(NSNotification*)notification {
  (void)notification;
  if (self.callback) self.callback(self.callbackContext, kQuit);
}

@end

namespace {

PliantEmbedderTestHost* Host(void* host) {
  return (__bridge PliantEmbedderTestHost*)host;
}

}  // namespace

extern "C" void* pliant_embedder_test_host_create(
    PliantEmbedderTestCallback callback,
    void* context,
    const char* url_a,
    const char* url_b) {
  ClearError();
  if (!NSApp || ![NSThread isMainThread] || !callback) {
    last_error = "host creation requires NSApp, the main thread, and a callback";
    Log(last_error);
    return nullptr;
  }
  @try {
    PliantEmbedderTestHost* host =
        [[PliantEmbedderTestHost alloc] initWithCallback:callback
                                                context:context
                                                   urlA:Text(url_a)
                                                   urlB:Text(url_b)];
    if (!host) {
      last_error = "host initializer returned nil";
      Log(last_error);
      return nullptr;
    }
    return (__bridge_retained void*)host;
  } @catch (NSException* exception) {
    RecordException("host creation", exception);
    return nullptr;
  }
}

extern "C" void pliant_embedder_test_host_destroy(void* host) {
  if (!host) return;
  @try {
    PliantEmbedderTestHost* controller = Host(host);
    controller.callback = nullptr;
    controller.window.delegate = nil;
    [controller.window close];
    CFBridgingRelease(host);
  } @catch (NSException* exception) {
    RecordException("host destruction", exception);
  }
}

extern "C" uint8_t pliant_embedder_test_host_show(void* host) {
  ClearError();
  if (!host) {
    last_error = "cannot show a null host";
    Log(last_error);
    return 0;
  }
  @try {
    [Host(host).window center];
    [Host(host).window orderFront:nil];
    return 1;
  } @catch (NSException* exception) {
    RecordException("host show", exception);
    return 0;
  }
}

extern "C" uint8_t pliant_embedder_test_host_set_status(void* host,
                                                          const char* text,
                                                          uint8_t is_error) {
  ClearError();
  if (!host) {
    last_error = "cannot set status on a null host";
    Log(last_error);
    return 0;
  }
  @try {
    Host(host).statusLabel.stringValue = Text(text);
    Host(host).statusLabel.textColor =
        is_error ? NSColor.systemRedColor : NSColor.labelColor;
    return 1;
  } @catch (NSException* exception) {
    RecordException("status update", exception);
    return 0;
  }
}

extern "C" uint8_t pliant_embedder_test_host_set_identity(void* host,
                                                            const char* text) {
  ClearError();
  if (!host) {
    last_error = "cannot set identity on a null host";
    Log(last_error);
    return 0;
  }
  @try {
    Host(host).identityLabel.stringValue = Text(text);
    return 1;
  } @catch (NSException* exception) {
    RecordException("identity update", exception);
    return 0;
  }
}

extern "C" void* pliant_embedder_test_host_content_container(void* host) {
  ClearError();
  if (!host) {
    last_error = "cannot get content container from a null host";
    Log(last_error);
    return nullptr;
  }
  @try {
    return (__bridge void*)Host(host).contentContainer;
  } @catch (NSException* exception) {
    RecordException("content container lookup", exception);
    return nullptr;
  }
}

extern "C" void pliant_embedder_test_host_set_diagnostic_log(
    const char* path) {
  diagnostic_path = path ? path : "";
  Log("diagnostic logging configured");
}

extern "C" const char* pliant_embedder_test_host_last_error() {
  return last_error.c_str();
}
