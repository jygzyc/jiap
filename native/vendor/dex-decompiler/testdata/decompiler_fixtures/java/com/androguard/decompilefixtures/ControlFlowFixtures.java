package com.androguard.decompilefixtures;

import java.util.Arrays;
import java.util.List;

/** if/else, loops, ternary, short-circuit, for-each. */
public final class ControlFlowFixtures {
    private ControlFlowFixtures() {}

    public static int ifElseChain(int x) {
        if (x < 0) {
            return -1;
        } else if (x == 0) {
            return 0;
        } else {
            return 1;
        }
    }

    public static boolean nestedIfAnd(boolean a, boolean b, boolean c) {
        if (a) {
            if (b && c) {
                return true;
            }
        }
        return false;
    }

    public static boolean shortCircuitOr(boolean a, boolean b) {
        if (a || b) {
            return true;
        }
        return false;
    }

    public static int assignTernary(int a, int b) {
        int max = a > b ? a : b;
        return max;
    }

    public static int whileLoop(int n) {
        int i = 0;
        int sum = 0;
        while (i < n) {
            sum += i;
            i++;
        }
        return sum;
    }

    public static int doWhileLoop(int n) {
        int i = 0;
        do {
            i++;
        } while (i < n);
        return i;
    }

    public static int forLoopClassic(int n) {
        int sum = 0;
        for (int i = 0; i < n; i++) {
            sum += i;
        }
        return sum;
    }

    public static int breakInLoop(int limit) {
        int sum = 0;
        for (int i = 0; i < 100; i++) {
            if (i >= limit) {
                break;
            }
            sum += i;
        }
        return sum;
    }

    public static int forEachArray(int[] arr) {
        int sum = 0;
        for (int v : arr) {
            sum += v;
        }
        return sum;
    }

    public static int forEachList(List<Integer> list) {
        int sum = 0;
        for (Integer v : list) {
            sum += v;
        }
        return sum;
    }

    public static int demoForEach() {
        return forEachArray(new int[] { 1, 2, 3 }) + forEachList(Arrays.asList(4, 5));
    }
}
