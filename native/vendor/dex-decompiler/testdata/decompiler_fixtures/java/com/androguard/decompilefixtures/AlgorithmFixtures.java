package com.androguard.decompilefixtures;

import java.util.ArrayDeque;
import java.util.Arrays;
import java.util.Queue;

/** Classic CS algorithms — loops, recursion, nested control flow, arrays. */
public final class AlgorithmFixtures {
    private AlgorithmFixtures() {}

    public static void bubbleSort(int[] arr) {
        int n = arr.length;
        for (int i = 0; i < n - 1; i++) {
            for (int j = 0; j < n - i - 1; j++) {
                if (arr[j] > arr[j + 1]) {
                    int tmp = arr[j];
                    arr[j] = arr[j + 1];
                    arr[j + 1] = tmp;
                }
            }
        }
    }

    public static int binarySearch(int[] arr, int target) {
        int lo = 0;
        int hi = arr.length - 1;
        while (lo <= hi) {
            int mid = lo + (hi - lo) / 2;
            if (arr[mid] == target) {
                return mid;
            } else if (arr[mid] < target) {
                lo = mid + 1;
            } else {
                hi = mid - 1;
            }
        }
        return -1;
    }

    public static int fibonacciIterative(int n) {
        if (n <= 1) {
            return n;
        }
        int a = 0;
        int b = 1;
        for (int i = 2; i <= n; i++) {
            int c = a + b;
            a = b;
            b = c;
        }
        return b;
    }

    public static int fibonacciRecursive(int n) {
        if (n <= 1) {
            return n;
        }
        return fibonacciRecursive(n - 1) + fibonacciRecursive(n - 2);
    }

    public static int gcdEuclid(int a, int b) {
        while (b != 0) {
            int t = b;
            b = a % b;
            a = t;
        }
        return Math.abs(a);
    }

    public static int[] sieveOfEratosthenes(int limit) {
        if (limit < 2) {
            return new int[0];
        }
        boolean[] composite = new boolean[limit + 1];
        for (int p = 2; p * p <= limit; p++) {
            if (!composite[p]) {
                for (int k = p * p; k <= limit; k += p) {
                    composite[k] = true;
                }
            }
        }
        int count = 0;
        for (int i = 2; i <= limit; i++) {
            if (!composite[i]) {
                count++;
            }
        }
        int[] primes = new int[count];
        int idx = 0;
        for (int i = 2; i <= limit; i++) {
            if (!composite[i]) {
                primes[idx++] = i;
            }
        }
        return primes;
    }

    public static void quickSort(int[] arr, int lo, int hi) {
        if (lo >= hi) {
            return;
        }
        int p = partition(arr, lo, hi);
        quickSort(arr, lo, p - 1);
        quickSort(arr, p + 1, hi);
    }

    private static int partition(int[] arr, int lo, int hi) {
        int pivot = arr[hi];
        int i = lo - 1;
        for (int j = lo; j < hi; j++) {
            if (arr[j] <= pivot) {
                i++;
                swap(arr, i, j);
            }
        }
        swap(arr, i + 1, hi);
        return i + 1;
    }

    private static void swap(int[] arr, int i, int j) {
        int tmp = arr[i];
        arr[i] = arr[j];
        arr[j] = tmp;
    }

    public static int[] mergeSort(int[] arr) {
        if (arr.length <= 1) {
            return arr;
        }
        int mid = arr.length / 2;
        int[] left = mergeSort(Arrays.copyOfRange(arr, 0, mid));
        int[] right = mergeSort(Arrays.copyOfRange(arr, mid, arr.length));
        return merge(left, right);
    }

    private static int[] merge(int[] left, int[] right) {
        int[] out = new int[left.length + right.length];
        int i = 0;
        int j = 0;
        int k = 0;
        while (i < left.length && j < right.length) {
            if (left[i] <= right[j]) {
                out[k++] = left[i++];
            } else {
                out[k++] = right[j++];
            }
        }
        while (i < left.length) {
            out[k++] = left[i++];
        }
        while (j < right.length) {
            out[k++] = right[j++];
        }
        return out;
    }

    /** BFS on adjacency matrix; returns distance from src to dst or -1. */
    public static int bfsShortestPath(int[][] graph, int src, int dst) {
        int n = graph.length;
        boolean[] visited = new boolean[n];
        int[] dist = new int[n];
        Arrays.fill(dist, -1);
        Queue<Integer> q = new ArrayDeque<>();
        q.add(src);
        visited[src] = true;
        dist[src] = 0;
        while (!q.isEmpty()) {
            int u = q.remove();
            if (u == dst) {
                return dist[u];
            }
            for (int v = 0; v < n; v++) {
                if (graph[u][v] != 0 && !visited[v]) {
                    visited[v] = true;
                    dist[v] = dist[u] + 1;
                    q.add(v);
                }
            }
        }
        return -1;
    }

    public static int demoAlgorithms() {
        int[] arr = { 5, 1, 4, 2, 8 };
        bubbleSort(arr);
        int idx = binarySearch(arr, 4);
        int fib = fibonacciIterative(10);
        int gcd = gcdEuclid(48, 18);
        int[] primes = sieveOfEratosthenes(20);
        quickSort(arr, 0, arr.length - 1);
        int[] sorted = mergeSort(new int[] { 3, 1, 4, 1, 5 });
        int[][] g = {
            { 0, 1, 0 },
            { 1, 0, 1 },
            { 0, 1, 0 }
        };
        int dist = bfsShortestPath(g, 0, 2);
        return idx + fib + gcd + primes.length + sorted.length + dist;
    }
}
