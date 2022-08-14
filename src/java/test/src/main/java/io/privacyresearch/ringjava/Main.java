package io.privacyresearch.ringjava;

import org.rust.CallId;
import org.rust.MyKey;
import org.rust.Opaque;
import org.rust.byte_array;
import org.rust.byte_array_2d;
import org.rust.JavaPlatform;
import static org.rust.tringlib_h.createCallEndpoint;
import static org.rust.tringlib_h.*;
import java.lang.invoke.VarHandle;
import java.lang.foreign.*;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.lang.foreign.ValueLayout.OfByte;
import java.lang.foreign.MemoryLayout.PathElement;
import java.util.Arrays;

public class Main {

    public static void main(String[] args) throws Exception {
        System.err.println("Hello, ring");
        Main main = new Main();
        main.flow2();
//        flow();
//        /
//System.err.println("CI = " + CallId.class);
//        try (MemorySession scope = MemorySession.openShared()) {
//            processByteArray2d(scope);
//             callback(scope);
//
//MemorySegment jByte = MemorySegment.ofArray(new byte[] {1,4,7});
//MemorySegment cByte = MemorySegment.allocateNative(3,scope);
//MemorySegment.copy(jByte, 0, cByte, 0, 3);
//System.err.println("invoke gmv...");
//gotMyVector(cByte, 3);
//
//System.err.println("invoked gmv...");
    }

    public void flow2() {
        MemorySession scope = MemorySession.openShared();
        long init = initRingRTC();
        System.err.println("Init RingRtc -> "+init);
        long answer = createCallEndpoint();
        System.err.println("ce = "+ answer);
        offerOnThread(answer);
        long offer = sendReceivedOffer(scope, answer, 975);
        offerOnThread(answer);
        long offer2 = sendReceivedOffer(scope, answer, 753);
        System.err.println("offer2 = "+ offer2);
        offerOnThread(answer);
    }

    long sendReceivedOffer(MemorySession scope, long callManagerId, int callId) {
        long offer = receivedOffer(callManagerId, callIdSegment(scope, callId));
        return offer;
    }

    MemorySegment callIdSegment(MemorySession scope, int callId) {
        MemorySegment callIdSegment = MemorySegment.allocateNative(8, scope);
        callIdSegment.set(ValueLayout.JAVA_LONG, 0l, callId);
        return callIdSegment;
    }

    void offerOnThread(long ce) {
        Thread t = new Thread() {
            @Override public void run() {
                long offer = 0;// receivedOffer(ce);
                System.err.println("In thread, offer = " + offer);
            }
        };
        t.start();
    }

    static void flow() {
        MemorySession scope = MemorySession.openShared();

        MemoryAddress javaPlatform = createJavaPlatform();
        System.err.println("Platform created at " + javaPlatform);
        long cm = createCallManager(javaPlatform.toRawLongValue());
        System.err.println("CM created: " + cm);
        long cm2 = createCallManager(javaPlatform.toRawLongValue());
        System.err.println("CM2 created: " + cm2);
        proceed(cm, 123, 1, 2);
        System.err.println("PROCEEDED!");
        try {
            Thread.sleep(5000);
        } catch (Exception e) {
            e.printStackTrace();
        }
//        
    }

    private static void callback(MemorySession scope) {
        byte[] opaque = new byte[]{34, 80, 10, 32, 133 - 256, 49, 219 - 256, 22, 137 - 256, 209 - 256, 73, 165 - 256, 83, 178 - 256, 90, 97, 228 - 256, 67, 88, 79, 71, 83, 83, 202 - 256, 101, 253 - 256, 38, 206 - 256, 67, 90, 106, 156 - 256, 249 - 256, 68, 174 - 256, 102, 18, 4, 54, 90, 100, 111, 26, 24, 49, 51, 54, 76, 86, 121, 103, 54, 122, 69, 119, 108, 118, 102, 119, 121, 69, 107, 107, 48, 69, 117, 68, 48, 34, 4, 8, 40, 16, 31, 34, 2, 8, 8, 40, 128 - 256, 137 - 256, 122, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0};
        scope = MemorySession.openShared();

        MemoryAddress javaPlatform = createJavaPlatform();
        System.err.println("Platform created at " + javaPlatform);
        long cm = createCallManager(javaPlatform.toRawLongValue());
        System.err.println("CM created: " + cm);
        proceed(cm, 123, 1, 2);
        System.err.println("PROCEEDED!");
//        
//
//        byte[] opaqueData = new byte[256];
//        byte[] mik = new byte[8];
//        mik[3] = 19;
//        byte[] remoteKey = new byte[8];
//        long remotePeer = 0l;
//        int deviceId = 0;
//        long ageSec = 0;
//        int cmt = 0;
//        int remoteDeviceId = 0;
//try {
//            MyStartCallback mscb = new MyStartCallback();
//            MemorySegment callbackSegment = JavaPlatform.startCallback.allocate(mscb, scope);
//            // VarHandle vh = JavaPlatform.startCallback$VH();
//            // System.err.println("VARH = " + vh);
//            System.err.println("memseg for callback  = " + callbackSegment);
//            //MemoryAddress ma = JavaPlatform.startCallback$get(callbackSegment);
//            MemoryAddress ma = MemoryAddress.ofLong(platform);
//            MemorySegment platformSegment = JavaPlatform.ofAddress(ma, scope);
//
//            System.err.println("ma = " + ma);
//            JavaPlatform.startCallback$set(platformSegment, callbackSegment.address());
//            JavaPlatform.bogusVal$set(platformSegment, 111);
//            System.err.println("Done setting callback!!");
//
//            set_first_callback(cm, (Addressable)callbackSegment);
//            System.err.println("Done setting callback2 to "+(Addressable)callbackSegment);
//
//            MemorySegment callid =  MemorySegment.allocateNative(8, scope); 
//            callid.set(ValueLayout.JAVA_LONG, 0l, 753l);
//            MemorySegment sik =  MemorySegment.ofArray(mik);
//            MemorySegment rik =  MemorySegment.ofArray(remoteKey);
//            // MemorySegment opaqueMs = MemorySegment.ofArray(opaque);
//            // MemorySegment opaqueData = MemorySegment.ofArray(opaque);
//            MemorySegment opaqueMs = Opaque.allocate(scope);
//            Opaque.len$set(opaqueMs, 82);
//            MemorySegment opaqueDataSegment = Opaque.data$slice(opaqueMs);
//            opaqueDataSegment.copyFrom(MemorySegment.ofArray(opaque));
//            //MemorySegment opaqueRaw = MemorySegment.allocateNative(256, scope);
//            // opaqueRaw.copyFrom(MemorySegment.ofArray(opaque));
//                // Opaque.rawdata$set(opaqueMs, opaqueRaw.address());
//                long off = received_offer(cm, callid, remotePeer, deviceId, opaqueMs, ageSec, cmt, remoteDeviceId, true, sik, rik);
//                System.err.println("received offer: "+ off);
//                Thread.sleep(3000);
//            } catch (Throwable sstt) {
//                System.err.println("Major throwable: " + sstt);
//                sstt.printStackTrace();
//            }
    }

    private void processByteArray(MemorySession scope) {
        byte[] raw = new byte[]{2, 5, 9, 11};
        MemorySegment bmem = nativeBytes(raw, scope);
        /*
 byte_array.allocate(scope);
        byte_array.length$set(bmem, raw.length);
        MemorySegment j1 = MemorySegment.ofArray(raw);
        MemorySegment n1 = MemorySegment.allocateNative(raw.length, scope);
        MemorySegment.copy(j1, 0, n1, 0, raw.length);
        byte_array.bytes$set(bmem, n1.address());
         */
//        gotMyVectors(bmem);
    }

    private static MemorySegment nativeBytes(byte[] raw, MemorySession scope) {
        MemorySegment bmem = byte_array.allocate(scope);
        byte_array.length$set(bmem, raw.length);
        MemorySegment j1 = MemorySegment.ofArray(raw);
        MemorySegment n1 = MemorySegment.allocateNative(raw.length, scope);
        MemorySegment.copy(j1, 0, n1, 0, raw.length);
        byte_array.bytes$set(bmem, n1.address());
        return bmem;
    }

    private static void processByteArray2d(MemorySession scope) {
        System.err.println("ProcessByteArray2d");
        int len = 3;
        MemorySegment mem2d = byte_array_2d.allocate(scope);
        MemorySegment rows = byte_array_2d.rows$slice(mem2d);
        byte_array_2d.length$set(mem2d, len);
        byte[][] raw = new byte[len][];
        for (int i = 0; i < len; i++) {
            int bl = (int) (1 + (5 * Math.random()));
            byte[] b = new byte[bl];
            for (int j = 0; j < bl; j++) {
                b[j] = (byte) (Math.random() * 256 - 256);
            }
            raw[i] = b;
            System.err.println("process row " + i + ", java : " + Arrays.toString(b));
            MemorySegment singleArray = nativeBytes(b, scope);
            MemorySegment.copy(singleArray, 0, rows, i * 16, 16);
            // MemorySegment m1item = byte_array.allocate(scope);
            // // byte_array_2d.bytes$set(mem2d, i, singleArray.address());
            // byte_array.bytes$set(mem1d, i, singleArray.address());
        }
        // byte_array_2d.bytes$set(mem2d, mem1d.address());
//        gotMyVectors2d(mem2d);
/*
        MemorySegment bmem = byte_array.allocate(scope);
        byte_array.length$set(bmem, raw.length);
        MemorySegment j1 = MemorySegment.ofArray(raw);
        MemorySegment n1 = MemorySegment.allocateNative(raw.length, scope);
        MemorySegment.copy(j1, 0, n1, 0, raw.length);
        byte_array.bytes$set(bmem, n1.address());
        gotMyVectors(bmem);
         */
    }

    static class MyStartCallback implements JavaPlatform.startCallback {

        public void apply(MemorySegment ms, long remotePeer, int dir, int mediaType) {
            try {
                System.err.println("Callback with memsegment: " + ms);
                System.err.println("class? " + CallId.class);
                long id = CallId.id$get(ms);
                System.err.println("IN JAVA, MYSTARTCALLBACK, call id = " + id + ", remotePeer = " + remotePeer + ", dir = " + dir + ", mediaType = " + mediaType);
            } catch (Throwable t) {
                t.printStackTrace();
            }
        }
    }

}
