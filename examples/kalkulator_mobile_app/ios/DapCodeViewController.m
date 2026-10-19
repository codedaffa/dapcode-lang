#import <UIKit/UIKit.h>
#import <GLKit/GLKit.h>
#import "dapcode_engine.h"

@interface DapCodeViewController : GLKViewController
@property (strong, nonatomic) EAGLContext *context;
@property (nonatomic) NSTimeInterval lastTime;
@end

@implementation DapCodeViewController

- (void)viewDidLoad {
    [super viewDidLoad];
    self.context = [[EAGLContext alloc] initWithAPI:kEAGLRenderingAPIOpenGLES2];
    GLKView *view = (GLKView *)self.view;
    view.context = self.context;
    view.drawableDepthFormat = GLKViewDrawableDepthFormat16;
    [EAGLContext setCurrentContext:self.context];
    self.preferredFramesPerSecond = 60;
    self.lastTime = [NSDate timeIntervalSinceReferenceDate];
    
    CGFloat scale = [UIScreen mainScreen].scale;
    dap_engine_init(view.bounds.size.width * scale, view.bounds.size.height * scale);
}

- (void)glkView:(GLKView *)view drawInRect:(CGRect)rect {
    NSTimeInterval now = [NSDate timeIntervalSinceReferenceDate];
    float dt = (float)(now - self.lastTime);
    if (dt <= 0.0f || dt > 0.1f) dt = 0.0166f;
    self.lastTime = now;
    
    dap_engine_step(dt);
}

- (void)touchesBegan:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event {
    UITouch *touch = [touches anyObject];
    CGPoint pt = [touch locationInView:self.view];
    CGFloat scale = [UIScreen mainScreen].scale;
    dap_engine_on_touch(pt.x * scale, pt.y * scale, 0);
}

- (void)touchesMoved:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event {
    UITouch *touch = [touches anyObject];
    CGPoint pt = [touch locationInView:self.view];
    CGFloat scale = [UIScreen mainScreen].scale;
    dap_engine_on_touch(pt.x * scale, pt.y * scale, 2);
}

- (void)touchesEnded:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event {
    UITouch *touch = [touches anyObject];
    CGPoint pt = [touch locationInView:self.view];
    CGFloat scale = [UIScreen mainScreen].scale;
    dap_engine_on_touch(pt.x * scale, pt.y * scale, 1);
}

@end
