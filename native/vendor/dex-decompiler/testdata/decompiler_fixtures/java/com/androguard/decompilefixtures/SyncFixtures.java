package com.androguard.decompilefixtures;

/** synchronized blocks and methods. */
public final class SyncFixtures {
    private SyncFixtures() {}

    private static final Object LOCK = new Object();
    private static int counter;

    public static synchronized int syncMethod(int delta) {
        counter += delta;
        return counter;
    }

    public static int syncBlock(int delta) {
        synchronized (LOCK) {
            counter += delta;
            return counter;
        }
    }

    public static int syncOnThis(SyncFixtures self, int delta) {
        synchronized (self) {
            return delta * 2;
        }
    }
}
