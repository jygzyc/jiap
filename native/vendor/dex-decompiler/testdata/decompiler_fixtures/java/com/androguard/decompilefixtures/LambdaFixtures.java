package com.androguard.decompilefixtures;

import java.util.function.IntUnaryOperator;

/** lambdas, method refs, indy-heavy patterns (Java 8). */
public final class LambdaFixtures {
    private LambdaFixtures() {}

    public static IntUnaryOperator identityLambda() {
        return x -> x + 1;
    }

    public static Runnable runnableLambda(final int n) {
        return () -> System.out.println(n);
    }

    public static int applyLambda(int x) {
        IntUnaryOperator op = identityLambda();
        return op.applyAsInt(x);
    }

    public static IntUnaryOperator methodRef() {
        return Math::abs;
    }

    public static int demoLambda() {
        return applyLambda(5) + methodRef().applyAsInt(-3);
    }
}
