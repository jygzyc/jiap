package com.androguard.demohunt;

import android.app.Activity;
import android.os.Bundle;

/** onCreate: setResult(RESULT_OK, getIntent()) (uri_permission_setresult_passthrough). */
public class ResultPassthroughActivity extends Activity {
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setResult(RESULT_OK, getIntent());
        finish();
    }
}
