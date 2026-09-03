package com.androguard.decompilefixtures;

/** inner / anonymous / local classes. */
public final class InnerFixtures {
    private InnerFixtures() {}

    public static class Outer {
        private int x = 1;

        public class Inner {
            public int bump() {
                return x + 1;
            }
        }

        public static class StaticInner {
            public int twice(int v) {
                return v * 2;
            }
        }
    }

    public static Runnable anonymousRunnable(final int seed) {
        return new Runnable() {
            @Override
            public void run() {
                System.out.println(seed);
            }
        };
    }

    public static int localClassCapture(final int base) {
        class Local {
            int value() {
                return base + 1;
            }
        }
        return new Local().value();
    }

    public static int demoInner() {
        Outer outer = new Outer();
        return outer.new Inner().bump()
                + Outer.StaticInner.class.hashCode()
                + localClassCapture(3);
    }
}
