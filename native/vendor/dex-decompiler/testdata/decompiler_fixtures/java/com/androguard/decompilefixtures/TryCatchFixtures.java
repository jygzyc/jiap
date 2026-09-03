package com.androguard.decompilefixtures;

import java.io.Closeable;
import java.io.IOException;

/** try/catch/finally, multi-catch, nested try, try-with-resources. */
public final class TryCatchFixtures {
    private TryCatchFixtures() {}

    static final class StubResource implements Closeable {
        private boolean closed;

        void touch() {
            if (closed) {
                throw new IllegalStateException("closed");
            }
        }

        @Override
        public void close() {
            closed = true;
        }
    }

    public static int simpleTryCatch(int x) {
        try {
            return 100 / x;
        } catch (ArithmeticException e) {
            return -1;
        }
    }

    private static volatile int finallyTouch;

    static void touchFinally() {
        // Opaque invoke — d8 must not inline this or it may drop try/finally ranges.
        if (finallyTouch != 0) {
            throw new AssertionError();
        }
    }

    public static int tryFinally(int x) {
        int acc = 0;
        try {
            acc += x;
            touchFinally();
        } finally {
            acc += 1;
        }
        return acc;
    }

    public static String multiCatch(Object o) {
        try {
            return o.toString();
        } catch (NullPointerException | ClassCastException e) {
            return "err";
        }
    }

    public static int nestedTry(int x) {
        try {
            try {
                return 100 / x;
            } catch (RuntimeException e) {
                return -1;
            }
        } catch (Exception e) {
            return -2;
        }
    }

    public static int tryWithResources() throws IOException {
        try (StubResource r = new StubResource()) {
            r.touch();
            return 42;
        }
    }

    public static int tryWithResourcesTwo() throws IOException {
        try (StubResource a = new StubResource(); StubResource b = new StubResource()) {
            a.touch();
            b.touch();
            return 7;
        }
    }
}
