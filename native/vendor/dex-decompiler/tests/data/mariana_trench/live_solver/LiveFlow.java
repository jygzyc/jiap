package mt.live;

import java.util.concurrent.Executor;

final class Origin {
    private Origin() {}

    static Object source() {
        return new Object();
    }

    static void sink(Object value) {
        // Modelled sink.
    }
}

/**
 * Small checked-in DEX counterpart to Mariana Trench's simple local-flow cases.
 */
public final class LiveFlow {
    private LiveFlow() {}

    public static void directFlow() {
        Object value = Origin.source();
        Origin.sink(value);
    }

    public static void cleanFlow() {
        Origin.sink(new Object());
    }

    private static Object identity(Object value) {
        return value;
    }

    public static void helperFlow() {
        Origin.sink(identity(Origin.source()));
    }

    public static void lambdaFlow() {
        final Object value = Origin.source();
        Runnable callback = () -> Origin.sink(value);
        callback.run();
    }

    public static void executorFlow(Executor executor) {
        final Object value = Origin.source();
        executor.execute(() -> Origin.sink(value));
    }
}
