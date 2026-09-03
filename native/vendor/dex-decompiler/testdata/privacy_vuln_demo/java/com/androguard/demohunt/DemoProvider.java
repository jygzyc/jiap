package com.androguard.demohunt;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.database.Cursor;
import android.database.sqlite.SQLiteDatabase;
import android.net.Uri;
import android.os.ParcelFileDescriptor;

import java.io.File;

/**
 * Exported grantUriPermissions provider.
 * query concatenates uri segment into rawQuery (provider_sql_injection).
 * openFile uses last path segment without canonicalize (provider_path_traversal).
 */
public class DemoProvider extends ContentProvider {
    @Override
    public boolean onCreate() {
        return true;
    }

    @Override
    public Cursor query(Uri uri, String[] projection, String selection,
                        String[] selectionArgs, String sortOrder) {
        try {
            String seg = uri.getLastPathSegment();
            SQLiteDatabase db = getContext().openOrCreateDatabase("provider.db", 0, null);
            // Direct extra → rawQuery so VF source_sink_scan sees the hop.
            return db.rawQuery(seg, null);
        } catch (Throwable ignored) {
            return null;
        }
    }

    @Override
    public android.os.ParcelFileDescriptor openFile(Uri uri, String mode) {
        try {
            File f = new File(getContext().getFilesDir(), uri.getLastPathSegment());
            return ParcelFileDescriptor.open(f, ParcelFileDescriptor.MODE_READ_ONLY);
        } catch (Throwable ignored) {
            return null;
        }
    }

    @Override
    public String getType(Uri uri) {
        return "vnd.android.cursor.dir/demo";
    }

    @Override
    public Uri insert(Uri uri, ContentValues values) {
        return uri;
    }

    @Override
    public int delete(Uri uri, String selection, String[] selectionArgs) {
        return 0;
    }

    @Override
    public int update(Uri uri, ContentValues values, String selection, String[] selectionArgs) {
        return 0;
    }
}
