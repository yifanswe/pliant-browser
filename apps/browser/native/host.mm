#include "host.h"

#import <AppKit/AppKit.h>

namespace {

enum HostEvent : uint32_t {
  kLoadDefinition = 0,
  kPreview = 1,
  kApply = 2,
  kReject = 3,
  kRestoreDefault = 4,
  kAddressSubmitted = 5,
  kButtonActivated = 6,
  kPageSelected = 7,
  kWindowClosed = 8,
  kPreviewStatus = 9,
  kRetrySave = 10,
};

NSString* Text(const char* value) {
  if (!value) return @"";
  return [NSString stringWithUTF8String:value] ?: @"";
}

}  // namespace

@interface PliantBrowserHostController : NSObject <NSWindowDelegate>
@property(nonatomic) PliantBrowserHostCallback callback;
@property(nonatomic) void* callbackContext;
@property(nonatomic) uint64_t generation;
@property(nonatomic, strong) NSWindow* window;
@property(nonatomic, strong) NSTextField* pathField;
@property(nonatomic, strong) NSTextField* statusLabel;
@property(nonatomic, strong) NSView* customRegion;
@property(nonatomic, strong) NSMutableDictionary<NSString*, NSView*>* nodes;
@property(nonatomic, strong) NSMutableArray<NSPopUpButton*>* pageLists;
@property(nonatomic, strong) NSMutableArray<NSTextField*>* addressFields;
@property(nonatomic, strong) NSView* contentSurface;
@property(nonatomic, strong) NSArray<NSButton*>* previewControls;
@property(nonatomic, strong) NSButton* retrySaveControl;
@property(nonatomic, strong) NSTimer* previewStatusTimer;
@property(nonatomic) BOOL previewMode;
- (instancetype)initWithCallback:(PliantBrowserHostCallback)callback
                         context:(void*)context;
- (void)emit:(HostEvent)kind
        page:(uint64_t)page
        node:(NSString*)node
       value:(NSString*)value
  generation:(uint64_t)generation;
- (void)previewStatusTick:(NSTimer*)timer;
@end

@implementation PliantBrowserHostController

- (instancetype)initWithCallback:(PliantBrowserHostCallback)callback
                         context:(void*)context {
  self = [super init];
  if (!self) return nil;
  _callback = callback;
  _callbackContext = context;
  _nodes = [NSMutableDictionary dictionary];
  _pageLists = [NSMutableArray array];
  _addressFields = [NSMutableArray array];

  _window = [[NSWindow alloc]
      initWithContentRect:NSMakeRect(0, 0, 1100, 760)
                styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
                          NSWindowStyleMaskMiniaturizable |
                          NSWindowStyleMaskResizable
                  backing:NSBackingStoreBuffered
                    defer:NO];
  _window.releasedWhenClosed = NO;
  _window.title = @"Pliant";
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

  NSStackView* controls = [NSStackView stackViewWithViews:@[]];
  controls.orientation = NSUserInterfaceLayoutOrientationHorizontal;
  controls.alignment = NSLayoutAttributeCenterY;
  controls.spacing = 6;
  NSTextField* pathLabel = [NSTextField labelWithString:@"Definition"];
  _pathField = [NSTextField textFieldWithString:@""];
  _pathField.placeholderString = @"Path to definition JSON";
  [_pathField setContentCompressionResistancePriority:NSLayoutPriorityDefaultLow
                                       forOrientation:NSLayoutConstraintOrientationHorizontal];
  [_pathField.widthAnchor constraintGreaterThanOrEqualToConstant:240].active = YES;
  [controls addArrangedSubview:pathLabel];
  [controls addArrangedSubview:_pathField];

  NSMutableArray<NSButton*>* buttons = [NSMutableArray array];
  for (NSArray<id>* spec in @[
         @[@"Load", @(kLoadDefinition)],
         @[@"Preview", @(kPreview)],
         @[@"Apply", @(kApply)],
         @[@"Reject", @(kReject)],
         @[@"Restore Default", @(kRestoreDefault)],
         @[@"Retry Save", @(kRetrySave)],
       ]) {
    NSButton* button = [NSButton buttonWithTitle:spec[0]
                                         target:self
                                         action:@selector(trustedAction:)];
    button.tag = [spec[1] integerValue];
    [buttons addObject:button];
    [controls addArrangedSubview:button];
  }
  _previewControls = buttons;
  _retrySaveControl = buttons.lastObject;
  _retrySaveControl.enabled = NO;

  _statusLabel = [NSTextField labelWithString:@"Starting browser..."];
  _statusLabel.lineBreakMode = NSLineBreakByTruncatingTail;
  [_statusLabel setContentCompressionResistancePriority:NSLayoutPriorityDefaultLow
                                        forOrientation:NSLayoutConstraintOrientationHorizontal];
  _customRegion = [[NSView alloc] initWithFrame:NSZeroRect];
  _customRegion.translatesAutoresizingMaskIntoConstraints = NO;
  [_customRegion.heightAnchor constraintGreaterThanOrEqualToConstant:420].active =
      YES;
  [root addArrangedSubview:controls];
  [root addArrangedSubview:_statusLabel];
  [root addArrangedSubview:_customRegion];
  [_customRegion.widthAnchor constraintEqualToAnchor:root.widthAnchor].active = YES;
  return self;
}

- (void)trustedAction:(NSButton*)sender {
  NSString* path = self.pathField.stringValue;
  [self emit:static_cast<HostEvent>(sender.tag)
        page:0
        node:@""
       value:path
  generation:self.generation];
}

- (void)addressSubmitted:(NSTextField*)sender {
  [self emit:kAddressSubmitted
        page:0
        node:sender.identifier
       value:sender.stringValue
  generation:static_cast<uint64_t>(sender.tag)];
}

- (void)buttonActivated:(NSButton*)sender {
  [self emit:kButtonActivated
        page:0
        node:sender.identifier
       value:@""
  generation:static_cast<uint64_t>(sender.tag)];
}

- (void)pageSelected:(NSPopUpButton*)sender {
  NSNumber* page = sender.selectedItem.representedObject;
  [self emit:kPageSelected
        page:page.unsignedLongLongValue
        node:sender.identifier
       value:@""
  generation:static_cast<uint64_t>(sender.tag)];
}

- (void)emit:(HostEvent)kind
        page:(uint64_t)page
        node:(NSString*)node
       value:(NSString*)value
  generation:(uint64_t)generation {
  if (!self.callback) return;
  self.callback(self.callbackContext, kind, generation, page,
                node.UTF8String ?: "", value.UTF8String ?: "");
}

- (void)previewStatusTick:(NSTimer*)timer {
  (void)timer;
  [self emit:kPreviewStatus
        page:0
        node:@""
       value:@""
  generation:self.generation];
}

- (void)windowWillClose:(NSNotification*)notification {
  (void)notification;
  [self.previewStatusTimer invalidate];
  self.previewStatusTimer = nil;
  [self emit:kWindowClosed
        page:0
        node:@""
       value:@""
  generation:self.generation];
}

@end

namespace {

PliantBrowserHostController* Host(void* host) {
  return (__bridge PliantBrowserHostController*)host;
}

NSStackView* Parent(PliantBrowserHostController* host, const char* parent) {
  NSView* view = host.nodes[Text(parent)];
  return [view isKindOfClass:[NSStackView class]] ? (NSStackView*)view : nil;
}

void StretchInCrossAxis(NSStackView* parent, NSView* view) {
  if (parent.orientation == NSUserInterfaceLayoutOrientationHorizontal) {
    [view.topAnchor constraintEqualToAnchor:parent.topAnchor].active = YES;
    [view.bottomAnchor constraintEqualToAnchor:parent.bottomAnchor].active = YES;
  } else {
    [view.leadingAnchor constraintEqualToAnchor:parent.leadingAnchor].active = YES;
    [view.trailingAnchor constraintEqualToAnchor:parent.trailingAnchor].active =
        YES;
  }
}

int Add(PliantBrowserHostController* host,
        const char* parent,
        const char* node_id,
        NSView* view) {
  if (!host || !node_id || host.nodes[Text(node_id)]) return 1;
  NSStackView* stack = Parent(host, parent);
  if (!stack) return 1;
  view.translatesAutoresizingMaskIntoConstraints = NO;
  view.identifier = Text(node_id);
  host.nodes[Text(node_id)] = view;
  [stack addArrangedSubview:view];
  return 0;
}

}  // namespace

extern "C" void* pliant_browser_host_create(
    PliantBrowserHostCallback callback,
    void* context) {
  if (!NSApp || ![NSThread isMainThread] || !callback) return nullptr;
  PliantBrowserHostController* host =
      [[PliantBrowserHostController alloc] initWithCallback:callback
                                                    context:context];
  return (__bridge_retained void*)host;
}

extern "C" void pliant_browser_host_destroy(void* host) {
  if (!host) return;
  PliantBrowserHostController* controller = Host(host);
  controller.callback = nullptr;
  [controller.previewStatusTimer invalidate];
  controller.previewStatusTimer = nil;
  controller.window.delegate = nil;
  [controller.window close];
  CFBridgingRelease(host);
}

extern "C" void pliant_browser_host_show(void* host) {
  if (!host) return;
  [Host(host).window center];
  [Host(host).window makeKeyAndOrderFront:nil];
}

extern "C" void pliant_browser_host_set_preview_mode(void* host,
                                                       uint8_t preview_mode) {
  if (!host) return;
  PliantBrowserHostController* controller = Host(host);
  controller.previewMode = preview_mode != 0;
  controller.window.title = preview_mode ? @"Pliant — Isolated Preview" : @"Pliant";
  for (NSButton* button in controller.previewControls) {
    button.enabled = !preview_mode;
  }
  controller.pathField.enabled = !preview_mode;
  if (!preview_mode) {
    for (NSButton* button in controller.previewControls) {
      if (button.tag == kApply || button.tag == kReject) button.enabled = NO;
    }
    controller.retrySaveControl.enabled = NO;
  }
}

extern "C" void pliant_browser_host_set_preview_pending(void* host,
                                                          uint8_t pending) {
  if (!host) return;
  for (NSButton* button in Host(host).previewControls) {
    if (button.tag == kApply || button.tag == kReject) {
      button.enabled = pending != 0 && !Host(host).previewMode;
    }
  }
}

extern "C" void pliant_browser_host_set_preview_checking(void* host,
                                                           uint8_t checking) {
  if (!host) return;
  PliantBrowserHostController* controller = Host(host);
  [controller.previewStatusTimer invalidate];
  controller.previewStatusTimer = nil;
  if (checking && !controller.previewMode) {
    controller.previewStatusTimer =
        [NSTimer scheduledTimerWithTimeInterval:0.05
                                        target:controller
                                      selector:@selector(previewStatusTick:)
                                      userInfo:nil
                                       repeats:YES];
  }
}

extern "C" void pliant_browser_host_set_save_pending(void* host,
                                                       uint8_t pending) {
  if (!host) return;
  PliantBrowserHostController* controller = Host(host);
  controller.retrySaveControl.enabled = pending != 0 && !controller.previewMode;
}

extern "C" void pliant_browser_host_set_definition_path(void* host,
                                                          const char* path) {
  if (host) Host(host).pathField.stringValue = Text(path);
}

extern "C" void pliant_browser_host_set_status(void* host,
                                                const char* text,
                                                uint8_t is_error) {
  if (!host) return;
  NSTextField* label = Host(host).statusLabel;
  label.stringValue = Text(text);
  label.textColor = is_error ? NSColor.systemRedColor
                             : NSColor.secondaryLabelColor;
  label.toolTip = label.stringValue;
}

extern "C" int pliant_browser_host_begin_layout(void* host,
                                                 uint64_t generation) {
  if (!host) return 1;
  PliantBrowserHostController* controller = Host(host);
  for (NSView* view in controller.customRegion.subviews.copy) {
    [view removeFromSuperview];
  }
  [controller.nodes removeAllObjects];
  [controller.pageLists removeAllObjects];
  [controller.addressFields removeAllObjects];
  controller.generation = generation;
  return 0;
}

extern "C" int pliant_browser_host_add_container(
    void* opaque,
    const char* parent,
    const char* node_id,
    uint8_t axis,
    double gap) {
  if (!opaque || !node_id) return 1;
  PliantBrowserHostController* host = Host(opaque);
  NSStackView* stack = [NSStackView stackViewWithViews:@[]];
  stack.orientation = axis == 0 ? NSUserInterfaceLayoutOrientationHorizontal
                                : NSUserInterfaceLayoutOrientationVertical;
  stack.alignment = axis == 0 ? NSLayoutAttributeCenterY
                              : NSLayoutAttributeLeading;
  stack.spacing = gap;
  stack.translatesAutoresizingMaskIntoConstraints = NO;
  stack.identifier = Text(node_id);
  host.nodes[Text(node_id)] = stack;
  if (!parent || parent[0] == '\0') {
    [host.customRegion addSubview:stack];
    [NSLayoutConstraint activateConstraints:@[
      [stack.leadingAnchor constraintEqualToAnchor:host.customRegion.leadingAnchor],
      [stack.trailingAnchor constraintEqualToAnchor:host.customRegion.trailingAnchor],
      [stack.topAnchor constraintEqualToAnchor:host.customRegion.topAnchor],
      [stack.bottomAnchor constraintEqualToAnchor:host.customRegion.bottomAnchor],
    ]];
    return 0;
  }
  NSStackView* parent_stack = Parent(host, parent);
  if (!parent_stack) return 1;
  [parent_stack addArrangedSubview:stack];
  [stack setContentHuggingPriority:NSLayoutPriorityDefaultLow
                   forOrientation:NSLayoutConstraintOrientationHorizontal];
  [stack setContentHuggingPriority:NSLayoutPriorityDefaultLow
                   forOrientation:NSLayoutConstraintOrientationVertical];
  StretchInCrossAxis(parent_stack, stack);
  return 0;
}

extern "C" int pliant_browser_host_add_spacer(void* opaque,
                                               const char* parent,
                                               const char* node_id,
                                               double size) {
  if (!opaque) return 1;
  PliantBrowserHostController* host = Host(opaque);
  NSView* spacer = [[NSView alloc] initWithFrame:NSZeroRect];
  int result = Add(host, parent, node_id, spacer);
  if (result != 0) return result;
  NSStackView* stack = Parent(host, parent);
  NSLayoutConstraint* constraint =
      stack.orientation == NSUserInterfaceLayoutOrientationHorizontal
          ? [spacer.widthAnchor constraintEqualToConstant:size]
          : [spacer.heightAnchor constraintEqualToConstant:size];
  constraint.active = YES;
  return 0;
}

extern "C" int pliant_browser_host_add_label(void* opaque,
                                              const char* parent,
                                              const char* node_id,
                                              const char* text) {
  if (!opaque) return 1;
  NSTextField* label = [NSTextField labelWithString:Text(text)];
  label.lineBreakMode = NSLineBreakByTruncatingTail;
  return Add(Host(opaque), parent, node_id, label);
}

extern "C" int pliant_browser_host_add_address(void* opaque,
                                                const char* parent,
                                                const char* node_id,
                                                const char* placeholder) {
  if (!opaque) return 1;
  PliantBrowserHostController* host = Host(opaque);
  NSTextField* field = [NSTextField textFieldWithString:@""];
  field.placeholderString = Text(placeholder);
  field.target = host;
  field.action = @selector(addressSubmitted:);
  field.tag = static_cast<NSInteger>(host.generation);
  [host.addressFields addObject:field];
  [field.widthAnchor constraintGreaterThanOrEqualToConstant:180].active = YES;
  [field setContentCompressionResistancePriority:NSLayoutPriorityDefaultLow
                                  forOrientation:NSLayoutConstraintOrientationHorizontal];
  return Add(host, parent, node_id, field);
}

extern "C" int pliant_browser_host_add_page_list(void* opaque,
                                                  const char* parent,
                                                  const char* node_id) {
  if (!opaque) return 1;
  PliantBrowserHostController* host = Host(opaque);
  NSPopUpButton* list = [[NSPopUpButton alloc] initWithFrame:NSZeroRect
                                                  pullsDown:NO];
  list.target = host;
  list.action = @selector(pageSelected:);
  list.tag = static_cast<NSInteger>(host.generation);
  [host.pageLists addObject:list];
  return Add(host, parent, node_id, list);
}

extern "C" int pliant_browser_host_add_content(void* opaque,
                                                const char* parent,
                                                const char* node_id) {
  if (!opaque) return 1;
  PliantBrowserHostController* host = Host(opaque);
  NSView* content = host.contentSurface;
  if (!content) {
    content = [[NSView alloc] initWithFrame:NSZeroRect];
    content.wantsLayer = YES;
    content.layer.backgroundColor = NSColor.windowBackgroundColor.CGColor;
    [content.widthAnchor constraintGreaterThanOrEqualToConstant:320].active =
        YES;
    [content.heightAnchor constraintGreaterThanOrEqualToConstant:320].active =
        YES;
    [content setContentHuggingPriority:NSLayoutPriorityDefaultLow
                       forOrientation:NSLayoutConstraintOrientationHorizontal];
    [content setContentHuggingPriority:NSLayoutPriorityDefaultLow
                       forOrientation:NSLayoutConstraintOrientationVertical];
  }
  int result = Add(host, parent, node_id, content);
  if (result == 0) {
    host.contentSurface = content;
    StretchInCrossAxis(Parent(host, parent), content);
  }
  return result;
}

extern "C" int pliant_browser_host_add_button(void* opaque,
                                               const char* parent,
                                               const char* node_id,
                                               const char* label) {
  if (!opaque) return 1;
  PliantBrowserHostController* host = Host(opaque);
  NSButton* button = [NSButton buttonWithTitle:Text(label)
                                        target:host
                                        action:@selector(buttonActivated:)];
  button.tag = static_cast<NSInteger>(host.generation);
  return Add(host, parent, node_id, button);
}

extern "C" int pliant_browser_host_end_layout(void* opaque) {
  if (!opaque) return 1;
  PliantBrowserHostController* host = Host(opaque);
  [host.window.contentView layoutSubtreeIfNeeded];
  return host.contentSurface ? 0 : 1;
}

extern "C" void pliant_browser_host_pages_begin(void* opaque) {
  if (!opaque) return;
  for (NSPopUpButton* list in Host(opaque).pageLists) {
    [list removeAllItems];
  }
}

extern "C" void pliant_browser_host_page_add(void* opaque,
                                              uint64_t page,
                                              const char* label,
                                              uint8_t selected) {
  if (!opaque) return;
  for (NSPopUpButton* list in Host(opaque).pageLists) {
    [list addItemWithTitle:Text(label)];
    list.lastItem.representedObject = @(page);
    if (selected) [list selectItem:list.lastItem];
  }
}

extern "C" void pliant_browser_host_pages_end(void* opaque) {
  if (!opaque) return;
  for (NSPopUpButton* list in Host(opaque).pageLists) {
    if (list.numberOfItems == 0) {
      [list addItemWithTitle:@"No open pages"];
      list.enabled = NO;
    } else {
      list.enabled = YES;
    }
  }
}

extern "C" void pliant_browser_host_set_current_url(void* opaque,
                                                     const char* url) {
  if (!opaque) return;
  for (NSTextField* field in Host(opaque).addressFields) {
    field.stringValue = Text(url);
  }
}

extern "C" void pliant_browser_host_set_node_enabled(void* opaque,
                                                      const char* node_id,
                                                      uint8_t enabled) {
  if (!opaque || !node_id) return;
  NSView* view = Host(opaque).nodes[Text(node_id)];
  if ([view respondsToSelector:@selector(setEnabled:)]) {
    [(id)view setEnabled:(enabled != 0)];
  }
}

extern "C" void* pliant_browser_host_content_container(void* opaque) {
  if (!opaque) return nullptr;
  return (__bridge void*)Host(opaque).contentSurface;
}
