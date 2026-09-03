package com.androguard.demohunt;

import android.app.Activity;
import android.os.Bundle;

/** StrandHogg bait: taskAffinity=com.evil.phish + allowTaskReparenting. */
public class TaskAffinityActivity extends Activity {
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
    }
}
