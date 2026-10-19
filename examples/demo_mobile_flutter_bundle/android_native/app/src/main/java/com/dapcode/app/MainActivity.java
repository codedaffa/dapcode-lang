package com.dapcode.app;

import android.app.Activity;
import android.os.Bundle;
import android.view.View;
import android.view.Window;
import android.view.WindowManager;
import com.dapcode.engine.DapCodeSurfaceView;

public class MainActivity extends Activity {
    private DapCodeSurfaceView surfaceView;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        requestWindowFeature(Window.FEATURE_NO_TITLE);
        getWindow().setFlags(
            WindowManager.LayoutParams.FLAG_FULLSCREEN,
            WindowManager.LayoutParams.FLAG_FULLSCREEN
        );
        getWindow().getDecorView().setSystemUiVisibility(
            View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
            | View.SYSTEM_UI_FLAG_FULLSCREEN
            | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
        );

        surfaceView = new DapCodeSurfaceView(this);
        setContentView(surfaceView);
    }

    @Override
    protected void onResume() {
        super.onResume();
        if (surfaceView != null) surfaceView.onResume();
    }

    @Override
    protected void onPause() {
        super.onPause();
        if (surfaceView != null) surfaceView.onPause();
    }
}
