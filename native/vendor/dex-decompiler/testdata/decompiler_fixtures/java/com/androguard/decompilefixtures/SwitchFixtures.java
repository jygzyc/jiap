package com.androguard.decompilefixtures;

/** packed/sparse/string/enum switches. */
public final class SwitchFixtures {
    private SwitchFixtures() {}

    public static int packedSwitch(int x) {
        switch (x) {
            case 1:
                return 10;
            case 2:
                return 20;
            case 3:
                return 30;
            case 4:
                return 40;
            case 5:
                return 50;
            default:
                return -1;
        }
    }

    public static String sparseSwitch(int x) {
        switch (x) {
            case 100:
                return "a";
            case 200:
                return "b";
            case 300:
                return "c";
            default:
                return "?";
        }
    }

    public static int switchOnString(String s) {
        switch (s) {
            case "alpha":
                return 1;
            case "beta":
                return 2;
            case "gamma":
                return 3;
            default:
                return 0;
        }
    }

    public static int switchOnEnum(EnumFixtures.Color c) {
        switch (c) {
            case RED:
                return 1;
            case GREEN:
                return 2;
            case BLUE:
                return 3;
            default:
                return 0;
        }
    }
}
