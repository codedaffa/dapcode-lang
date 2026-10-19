package com.dapcode.engine;

public class DapCodeEngine {
    static {
        System.loadLibrary("dapcode_app");
    }
    public static native void nativeInit(int width, int height);
    public static native void nativeResize(int width, int height);
    public static native void nativeRender(float dt);
    public static native void nativeTouch(float x, float y, int action);
    public static native void nativeKey(int keyCode, int isDown);
}
