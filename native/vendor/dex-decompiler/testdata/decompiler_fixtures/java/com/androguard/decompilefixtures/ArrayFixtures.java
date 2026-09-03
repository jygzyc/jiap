package com.androguard.decompilefixtures;

import java.util.Arrays;

/** filled-new-array, fill-array-data, aget/aput, new-array. */
public final class ArrayFixtures {
    private ArrayFixtures() {}

    public static int[] newIntArray() {
        return new int[] { 1, 2, 3, 4, 5 };
    }

    public static String[] newStringArray() {
        return new String[] { "a", "b", "c" };
    }

    public static int[] fillArrayData(int n) {
        int[] out = new int[n];
        for (int i = 0; i < n; i++) {
            out[i] = i * i;
        }
        return out;
    }

    public static int sum2d(int[][] m) {
        int sum = 0;
        for (int[] row : m) {
            for (int v : row) {
                sum += v;
            }
        }
        return sum;
    }

    public static int agetAput(int[] arr, int idx, int val) {
        arr[idx] = val;
        return arr[idx];
    }

    public static int demoArrays() {
        int[] a = newIntArray();
        int[] b = fillArrayData(4);
        int[][] m = new int[][] { a, b };
        return sum2d(m) + agetAput(Arrays.copyOf(a, a.length), 0, 99);
    }
}
