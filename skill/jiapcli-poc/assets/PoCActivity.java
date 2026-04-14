package com.poc.<target_app>;

import android.app.Activity;
import android.os.Bundle;
import android.util.Log;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;

import com.poc.<target_app>.exploit.Exploit;
import com.poc.<target_app>.exploit.Vuln1Exploit;
import com.poc.<target_app>.exploit.Vuln2Exploit;

public class PoCActivity extends Activity {

    private static final String TAG = "PoC";
    private TextView tvLog;
    private LinearLayout llButtons;

    // 所有 Exploit 注册在此，新增漏洞只需添加一行
    private static final Class<? extends Exploit>[] EXPLOITS = new Class[]{
        Vuln1Exploit.class,
        Vuln2Exploit.class,
    };

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_poc);

        tvLog = findViewById(R.id.tv_log);
        llButtons = findViewById(R.id.ll_buttons);

        for (Class<? extends Exploit> clazz : EXPLOITS) {
            Button btn = new Button(this);
            btn.setText(clazz.getSimpleName().replace("Exploit", ""));
            btn.setOnClickListener(v -> runExploit(clazz));
            llButtons.addView(btn);
        }
    }

    private void runExploit(Class<? extends Exploit> clazz) {
        log("─── " + clazz.getSimpleName() + " ───");
        try {
            Exploit exploit = clazz.getConstructor(Activity.class).newInstance(this);
            exploit.execute();
            log("执行完成");
        } catch (Exception e) {
            log("ERROR: " + e.getMessage());
            Log.e(TAG, "PoC failed", e);
        }
    }

    private void log(String msg) {
        Log.i(TAG, msg);
        tvLog.append(msg + "\n");
        ((ScrollView) tvLog.getParent()).fullScroll(View.FOCUS_DOWN);
    }
}
