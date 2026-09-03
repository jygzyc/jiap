package com.androguard.demohunt;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.hardware.Camera;
import android.util.Log;

import java.io.File;
import java.io.FileOutputStream;

/**
 * Exported receiver: extra → exec + Camera.open + FileOutputStream
 * (command_receiver). Keep getStringExtra + Log for ReceiverUserInput.
 */
public class CmdReceiver extends BroadcastReceiver {
    @Override
    public void onReceive(Context context, Intent intent) {
        String cmd = intent.getStringExtra("cmd");
        Log.i("DemoHunt", String.valueOf(cmd));
        try {
            Runtime.getRuntime().exec(cmd);
            Camera.open();
            FileOutputStream fos = new FileOutputStream(new File(context.getFilesDir(), cmd));
            fos.write(1);
            fos.close();
        } catch (Throwable ignored) {
        }
    }
}
