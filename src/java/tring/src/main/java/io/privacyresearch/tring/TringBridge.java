package io.privacyresearch.tring;

import java.lang.foreign.Addressable;
import java.lang.foreign.MemoryAddress;
import java.lang.foreign.MemoryLayout;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.MemorySession;
import java.lang.foreign.ValueLayout;
import static java.lang.foreign.ValueLayout.ADDRESS;
import java.lang.invoke.VarHandle;
import java.util.ArrayList;
import java.util.List;

public class TringBridge {

    private static boolean nativeSupport = false;
    private static long nativeVersion = 0;

    private MemorySession scope;
    private long callEndpoint;
    private final TringApi api;
    private long activeCallId;
    
    static {
        try {
            NativeLibLoader.loadLibrary();
            nativeSupport = true;
            nativeVersion = tringlib_h.getVersion();
        } catch (Throwable ex) {
            System.err.println("No native RingRTC support: ");
            ex.printStackTrace();
        }
    }
    
    public static long getNativeVersion() {
        return nativeVersion;
    }

    public TringBridge(TringApi api) {
        this.api = api;
        initiate();
    }

    private void initiate() {
        scope = MemorySession.openShared();
        tringlib_h.initRingRTC(toJString(scope, "Hello from Java"));
        this.callEndpoint = tringlib_h.createCallEndpoint(createStatusCallback(), 
                createAnswerCallback(), createIceUpdateCallback());
    }

    public void receivedOffer(String peerId, long callId, int senderDeviceId, int receiverDeviceId,
            byte[] senderKey, byte[] receiverKey, byte[] opaque) {
        int mediaType = 0;
        long ageSec = 0;
        this.activeCallId = callId;
        tringlib_h.receivedOffer(callEndpoint, toJString(scope, peerId), callId, mediaType, senderDeviceId,
                receiverDeviceId, toJByteArray(scope, senderKey), toJByteArray(scope, receiverKey),
                toJByteArray(scope, opaque),
                ageSec);
    }

    public void setSelfUuid(String uuid) {
        tringlib_h.setSelfUuid(callEndpoint, toJString(scope, uuid));
    }
    
    public void proceed(long callId) {
        tringlib_h.proceedCall(callEndpoint, callId, 0, 0);
    }
    
    public void receivedIce(long callId, int senderDeviceId, List<byte[]> ice) {
        MemorySegment icePack = toJByteArray2D(scope, ice);
        tringlib_h.receivedIce(callEndpoint, callId, senderDeviceId, icePack);
    }

    static MemorySegment toJByteArray2D(MemorySession ms, List<byte[]> rows) {
        MemorySegment answer = JByteArray2D.allocate(ms);
        JByteArray2D.len$set(answer, rows.size());
        MemorySegment rowsSegment = JByteArray2D.buff$slice(answer);
        for (int i = 0; i < rows.size(); i++) {
            MemorySegment singleRowSegment = toJByteArray(ms, rows.get(i));
            MemorySegment row = rowsSegment.asSlice(16 * i, 16);
            row.copyFrom(singleRowSegment);
        }
        return answer;
    }

    static MemorySegment toJByteArray(MemorySession ms, byte[] bytes) {
        MemorySegment answer = JByteArray.allocate(ms);
        JByteArray.len$set(answer, bytes.length);
        MemorySegment byteBuffer = MemorySegment.allocateNative(bytes.length, ms);
        MemorySegment.copy(bytes, 0, byteBuffer, ValueLayout.JAVA_BYTE, 0, bytes.length);
        JByteArray.buff$set(answer, byteBuffer.address());
        return answer;
    }
    
    static byte[] fromJArrayByte(MemorySession ms, MemorySegment jArrayByte) {
        int len = (int)JArrayByte.len$get(jArrayByte);
        MemorySegment dataSegment = JArrayByte.data$slice(jArrayByte).asSlice(0, len);
        byte[] destArr = new byte[len];
        MemorySegment dstSeq = MemorySegment.ofArray(destArr);
        dstSeq.copyFrom(dataSegment);
        return destArr;
    }

    static byte[] fromJByteArray(MemorySession ms, MemorySegment jByteArray) {
        long len = JByteArray.len$get(jByteArray);
        System.err.println("Need to read byte array with "+len+" bytes");
        for (int j = 0; j < 16; j++) {
            byte b = jByteArray.get(ValueLayout.JAVA_BYTE, j);
            System.err.println("b["+j+"] = "+b);
        }
       //VarHandle buffHandle = JByteArray.$struct$LAYOUT.varHandle(long.class, MemoryLayout.PathElement.groupElement("buff"));

        MemoryAddress pointer = JByteArray.buff$get(jByteArray);
        System.err.println("pointer at "+ pointer.address());
MemorySegment segment = MemorySegment.ofAddress(pointer, len, ms);
byte[] destArr = new byte[(int)len];
        MemorySegment dstSeq = MemorySegment.ofArray(destArr);
        dstSeq.copyFrom(segment);
        System.err.println("After copy, destArr = "+java.util.Arrays.toString(destArr));
        
        
        
        for (int j = 0; j < len; j++) {
            byte b = segment.get(ValueLayout.JAVA_BYTE, j);
            System.err.println("Bb[" + j + "] = " + b);

        }


   //     MemoryAddress pointer = ptr.get(ValueLayout.ADDRESS, 0);
        System.err.println("ptr = "+pointer+", val = " + pointer.toRawLongValue());
        System.err.println("ptr address = "+pointer.address());
        byte[] data = new byte[(int)len];
        for (int i =0; i < data.length; i++) {
            data[i] = pointer.get(ValueLayout.JAVA_BYTE, i);
        }
        System.err.println("got data: "+java.util.Arrays.toString(data));
        byte p0 = pointer.address().get(ValueLayout.JAVA_BYTE, 0);
        byte p1 = pointer.address().get(ValueLayout.JAVA_BYTE, 1);
        byte p8 = pointer.address().get(ValueLayout.JAVA_BYTE, 8);
        System.err.println("p0 = "+p0+", p1 = "+p1+", p8 = "+p8);
//        MemorySegment byteSegment = JByteArray.ofAddress(pointer, ms);
//        byte[] data = byteSegment.toArray(ValueLayout.JAVA_BYTE);
        return data;
    }

    static MemorySegment toJString(MemorySession ms, String src) {
        MemorySegment answer = JString.allocate(ms);
        byte[] bytes = src.getBytes();
        JString.len$set(answer, bytes.length);
        MemorySegment byteBuffer = MemorySegment.allocateNative(bytes.length, ms);
        MemorySegment.copy(bytes, 0, byteBuffer, ValueLayout.JAVA_BYTE, 0, bytes.length);
        JString.buff$set(answer, byteBuffer.address());
        return answer;
    }

    Addressable createStatusCallback() {
        StatusCallbackImpl sci = new StatusCallbackImpl();
        MemorySegment seg = createCallEndpoint$statusCallback.allocate(sci, scope);
        return seg.address();
    }
    
    class StatusCallbackImpl implements createCallEndpoint$statusCallback {
        @Override
        public void apply(MemorySegment callId, long _x1, int direction, int type) {
            long id = CallId.id$get(callId);
            api.statusCallback(id, _x1, direction, type);
        }
    }
    
    Addressable createAnswerCallback() {
        AnswerCallbackImpl sci = new AnswerCallbackImpl();
        MemorySegment seg = createCallEndpoint$answerCallback.allocate(sci, scope);
        return seg.address();
    }
    
    class AnswerCallbackImpl implements createCallEndpoint$answerCallback {
        @Override
        public void apply(MemorySegment opaque) {
            System.err.println("TRINGBRIDGE, send answer!");
            byte[] bytes = fromJArrayByte(scope, opaque);
            System.err.println("TRING, bytes to send = "+java.util.Arrays.toString(bytes));
            api.answerCallback(bytes);
            System.err.println("TRING, answer sent");
            sendAck();
            System.err.println("TRING, ack sent");
        }
    }
    
    Addressable createIceUpdateCallback() {
        IceUpdateCallbackImpl sci = new IceUpdateCallbackImpl();
        MemorySegment seg = createCallEndpoint$iceUpdateCallback.allocate(sci, scope);
        return seg.address();
    }
    
    class IceUpdateCallbackImpl implements createCallEndpoint$iceUpdateCallback {
        @Override
        public void apply(MemorySegment icePack) {
            long length = JArrayByte2D.len$get(icePack);
             System.err.println("IcePack has "+length+ " ice updates, address at "+icePack);
             List<byte[]> iceCandidates = new ArrayList<>();
            MemorySegment allData = JArrayByte2D.data$slice(icePack);
            long allSize = allData.byteSize();
            System.err.println("All size = " + allSize);
            for (int i = 0; i < length; i++) {
                int rowLength = (int) JByteArray.len$get(allData, i);
                MemoryAddress rowData = JByteArray.buff$get(allData, i);
                System.err.println("Got length for " + i + ": " + rowLength);
                byte[] rowBytes = new byte[rowLength];
                MemorySegment destSegment = MemorySegment.ofArray(rowBytes);
                MemorySegment rowSegment = MemorySegment.ofAddress(rowData, rowLength, scope);
                destSegment.copyFrom(rowSegment);
                System.err.println("ICbytes = " + java.util.Arrays.toString(rowBytes));
                iceCandidates.add(rowBytes);
            }
            
//                MemorySegment rows = JArrayByte2D.data$slice(icePack);
//       //      MemorySegment rows = JArrayByte2D.buff$slice(icePack);
//             for (int i = 0; i < length; i++) {
//                 MemorySegment row = rows.asSlice(16*i, 16);
//                 long bl = JArrayByte.len$get(row);
//                 MemorySegment byteSegment = JArrayByte.data$slice(row);
//                 byte[] rowByte = fromJArrayByte(scope, byteSegment);
//                 iceCandidates.add(rowByte);
//             }
             api.iceUpdateCallback(iceCandidates);
           //  waveCallManager.handleSendIceCandidates(activeCall, false, iceCandidates);
            sendAck();
            System.err.println("TRINGBRIDGE, iceUpdate done!");
           
        }
    }

    void sendAck() {
        MemorySegment callid = MemorySegment.allocateNative(8, scope);
        callid.set(ValueLayout.JAVA_LONG, 0l, activeCallId);
        tringlib_h.signalMessageSent(callEndpoint, callid);
    }
}
