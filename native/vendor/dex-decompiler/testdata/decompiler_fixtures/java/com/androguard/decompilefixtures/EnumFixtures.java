package com.androguard.decompilefixtures;

/** Real enum emission + $values / ordinal switch patterns. */
public final class EnumFixtures {
    private EnumFixtures() {}

    public enum Color {
        RED,
        GREEN,
        BLUE,
        YELLOW,
        ORANGE
    }

    public static Color[] allColors() {
        return Color.values();
    }

    public static int ordinalOf(Color c) {
        return c.ordinal();
    }
}
