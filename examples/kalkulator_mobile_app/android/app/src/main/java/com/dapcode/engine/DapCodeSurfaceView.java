package com.dapcode.engine;

import android.content.Context;
import android.opengl.GLSurfaceView;
import android.view.MotionEvent;
import android.view.KeyEvent;
import javax.microedition.khronos.egl.EGLConfig;
import javax.microedition.khronos.opengles.GL10;

public class DapCodeSurfaceView extends GLSurfaceView implements GLSurfaceView.Renderer {
    private long lastFrameTime = 0;

    public DapCodeSurfaceView(Context context) {
        super(context);
        setEGLContextClientVersion(2);
        setEGLConfigChooser(8, 8, 8, 8, 16, 0);
        setRenderer(this);
        setRenderMode(RENDERMODE_CONTINUOUSLY);
    }

    @Override
    public void onSurfaceCreated(GL10 gl, EGLConfig config) {
        lastFrameTime = System.nanoTime();
        DapCodeEngine.nativeInit(getWidth(), getHeight());
    }

    @Override
    public void onSurfaceChanged(GL10 gl, int width, int height) {
        DapCodeEngine.nativeResize(width, height);
    }

    @Override
    public void onDrawFrame(GL10 gl) {
        long now = System.nanoTime();
        float dt = (now - lastFrameTime) / 1000000000.0f;
        if (dt <= 0.0f || dt > 0.1f) dt = 0.0166f;
        lastFrameTime = now;
        DapCodeEngine.nativeRender(dt);
    }

    @Override
    public boolean onTouchEvent(MotionEvent event) {
        int action = event.getActionMasked();
        int act = 2; // move
        if (action == MotionEvent.ACTION_DOWN) act = 0;
        else if (action == MotionEvent.ACTION_UP || action == MotionEvent.ACTION_CANCEL) act = 1;
        DapCodeEngine.nativeTouch(event.getX(), event.getY(), act);
        return true;
    }

    @Override
    public boolean onKeyDown(int keyCode, KeyEvent event) {
        DapCodeEngine.nativeKey(keyCode, 1);
        return super.onKeyDown(keyCode, event);
    }

    @Override
    public boolean onKeyUp(int keyCode, KeyEvent event) {
        DapCodeEngine.nativeKey(keyCode, 0);
        return super.onKeyUp(keyCode, event);
    }
}
