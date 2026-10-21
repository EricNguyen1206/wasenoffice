#import <Cocoa/Cocoa.h>

typedef void (*TouchBarCallback)(int index, const char *action);

@interface WasenTouchBarDelegate : NSObject <NSTouchBarDelegate>
@property (nonatomic, strong) NSArray<NSString *> *tabTitles;
@property (nonatomic, assign) NSInteger activeIndex;
@property (nonatomic, assign) TouchBarCallback callback;
@end

@implementation WasenTouchBarDelegate

- (NSTouchBarItem *)touchBar:(NSTouchBar *)touchBar makeItemForIdentifier:(NSTouchBarItemIdentifier)identifier {
    if ([identifier isEqualToString:@"com.wasenoffice.touchbar.new"]) {
        NSCustomTouchBarItem *item = [[NSCustomTouchBarItem alloc] initWithIdentifier:identifier];
        NSButton *btn = [NSButton buttonWithTitle:@"+ Mở/Tạo" target:self action:@selector(onNewTab:)];
        item.view = btn;
        return item;
    }
    if ([identifier isEqualToString:@"com.wasenoffice.touchbar.tabs"]) {
        NSCustomTouchBarItem *item = [[NSCustomTouchBarItem alloc] initWithIdentifier:identifier];
        if (!self.tabTitles || self.tabTitles.count == 0) {
            NSTextField *label = [NSTextField labelWithString:@"WasenOffice"];
            [label setTextColor:[NSColor secondaryLabelColor]];
            item.view = label;
            return item;
        }
        NSSegmentedControl *seg = [NSSegmentedControl segmentedControlWithLabels:self.tabTitles
                                                                   trackingMode:NSSegmentSwitchTrackingSelectOne
                                                                         target:self
                                                                         action:@selector(onTabSelected:)];
        if (self.activeIndex >= 0 && self.activeIndex < (NSInteger)self.tabTitles.count) {
            [seg setSelectedSegment:self.activeIndex];
        }
        item.view = seg;
        return item;
    }
    if ([identifier isEqualToString:@"com.wasenoffice.touchbar.save"]) {
        NSCustomTouchBarItem *item = [[NSCustomTouchBarItem alloc] initWithIdentifier:identifier];
        NSButton *btn = [NSButton buttonWithTitle:@"💾 Lưu" target:self action:@selector(onSave:)];
        item.view = btn;
        return item;
    }
    if ([identifier isEqualToString:@"com.wasenoffice.touchbar.close"]) {
        NSCustomTouchBarItem *item = [[NSCustomTouchBarItem alloc] initWithIdentifier:identifier];
        NSButton *btn = [NSButton buttonWithTitle:@"✕ Đóng" target:self action:@selector(onClose:)];
        item.view = btn;
        return item;
    }
    return nil;
}

- (void)onTabSelected:(NSSegmentedControl *)sender {
    if (self.callback) {
        self.callback((int)sender.selectedSegment, "select");
    }
}

- (void)onNewTab:(id)sender {
    if (self.callback) {
        self.callback(-1, "new");
    }
}

- (void)onSave:(id)sender {
    if (self.callback) {
        self.callback((int)self.activeIndex, "save");
    }
}

- (void)onClose:(id)sender {
    if (self.callback) {
        self.callback((int)self.activeIndex, "close");
    }
}

@end

static WasenTouchBarDelegate *g_delegate = nil;
static NSTouchBar *g_touchBar = nil;
static __weak NSWindow *g_window = nil;

void wasen_touchbar_init(void *ns_window_ptr, TouchBarCallback callback) {
    if (!ns_window_ptr) return;
    dispatch_async(dispatch_get_main_queue(), ^{
        NSWindow *window = (__bridge NSWindow *)ns_window_ptr;
        g_window = window;
        if (!g_delegate) {
            g_delegate = [[WasenTouchBarDelegate alloc] init];
        }
        g_delegate.callback = callback;
        
        g_touchBar = [[NSTouchBar alloc] init];
        g_touchBar.delegate = g_delegate;
        g_touchBar.defaultItemIdentifiers = @[
            @"com.wasenoffice.touchbar.new",
            @"com.wasenoffice.touchbar.tabs",
            @"com.wasenoffice.touchbar.save",
            @"com.wasenoffice.touchbar.close"
        ];
        
        if ([window respondsToSelector:@selector(setTouchBar:)]) {
            [window setTouchBar:g_touchBar];
        }
    });
}

void wasen_touchbar_update_tabs(const char **titles, int count, int active_index) {
    NSMutableArray<NSString *> *arr = [NSMutableArray arrayWithCapacity:count];
    for (int i = 0; i < count; i++) {
        if (titles[i]) {
            [arr addObject:[NSString stringWithUTF8String:titles[i]]];
        }
    }
    dispatch_async(dispatch_get_main_queue(), ^{
        if (!g_delegate) {
            g_delegate = [[WasenTouchBarDelegate alloc] init];
        }
        g_delegate.tabTitles = arr;
        g_delegate.activeIndex = active_index;
        
        if (g_touchBar && g_window) {
            // Re-assign touchbar to refresh items
            g_touchBar.delegate = nil;
            g_touchBar.delegate = g_delegate;
            if ([g_window respondsToSelector:@selector(setTouchBar:)]) {
                [g_window setTouchBar:nil];
                [g_window setTouchBar:g_touchBar];
            }
        }
    });
}
