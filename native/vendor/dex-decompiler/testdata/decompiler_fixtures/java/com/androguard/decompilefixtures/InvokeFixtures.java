package com.androguard.decompilefixtures;

import android.content.Context;

/** invoke chains, move-result, null checks, static/instance calls. */
public final class InvokeFixtures {
    private InvokeFixtures() {}

    public static String chainedStringOps(String s) {
        return s.trim().toLowerCase().substring(0, 1);
    }

    public static int staticMath(int a, int b) {
        return Math.max(a, Math.min(b, a + b));
    }

    public static boolean nullCheckContext(Context ctx) {
        if (ctx == null) {
            return false;
        }
        return ctx.getPackageName() != null;
    }

    public static int invokeResultInCondition(Context ctx) {
        Object svc = ctx != null ? ctx.getSystemService(Context.ACTIVITY_SERVICE) : null;
        if (svc != null) {
            return svc.hashCode();
        }
        return -1;
    }

    public static String builderChain() {
        StringBuilder sb = new StringBuilder();
        sb.append("a");
        sb.append("b");
        sb.append("c");
        return sb.toString();
    }
}
