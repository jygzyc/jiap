package com.androguard.decompilefixtures;

/** filled-new-array + static field refs (enum-like $values pattern). */
public final class NamingFixtures {
    private NamingFixtures() {}

    static final class Token {
        static final Token A = new Token("A");
        static final Token B = new Token("B");
        static final Token C = new Token("C");
        static final Token D = new Token("D");
        static final Token E = new Token("E");

        private final String name;

        Token(String name) {
            this.name = name;
        }

        String label() {
            return name;
        }
    }

    /** Mirrors MSAL-style $values(): locals + sget operands in one array literal. */
    public static Token[] tokenValuesArray() {
        Token a = Token.A;
        Token b = Token.B;
        Token c = Token.C;
        return new Token[] { a, b, c, Token.D, Token.E };
    }

    public static int nullCheckAfterInvoke(ContextLike ctx) {
        Object svc = ctx.getService("test");
        if (svc != null) {
            return svc.hashCode();
        }
        return 0;
    }

    public interface ContextLike {
        Object getService(String name);
    }
}
