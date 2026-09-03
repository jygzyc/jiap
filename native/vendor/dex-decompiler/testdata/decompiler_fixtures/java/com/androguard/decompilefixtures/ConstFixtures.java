package com.androguard.decompilefixtures;

/** const-string, wide const, type casts, instanceof. */
public final class ConstFixtures {
    private ConstFixtures() {}

    public static long wideConst() {
        return 0x1234567890ABCDEFL;
    }

    public static double doubleConst() {
        return 3.141592653589793;
    }

    public static String manyStrings() {
        return "one" + "two" + "three" + "four";
    }

    public static int castsAndInstanceof(Object o) {
        if (o instanceof Number) {
            return ((Number) o).intValue();
        }
        if (o instanceof CharSequence) {
            return ((CharSequence) o).length();
        }
        return -1;
    }
}
