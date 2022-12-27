package io.privacyresearch.tring;

import io.privacyresearch.tringapi.TringFrame;
import io.privacyresearch.tringapi.TringService;
import java.lang.foreign.Addressable;
import java.lang.foreign.MemoryAddress;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.MemorySession;
import java.lang.foreign.ValueLayout;
import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.logging.Level;
import java.util.logging.Logger;

public class TringServiceImpl implements TringService {

    static final int BANDWIDTH_QUALITY_HIGH = 2;
    private static final TringService instance = new TringServiceImpl();
    private static boolean nativeSupport = false;
    private static long nativeVersion = 0;

    private MemorySession scope;
    private long callEndpoint;
    private io.privacyresearch.tringapi.TringApi api;
    private long activeCallId;
    static String libName = "unknown";
    BlockingQueue<TringFrame> frameQueue = new LinkedBlockingQueue();

    private static final Logger LOG = Logger.getLogger(TringServiceImpl.class.getName());

    static {
        try {
            libName = NativeLibLoader.loadLibrary();
            nativeSupport = true;
            nativeVersion = tringlib_h.getVersion();
            
        } catch (Throwable ex) {
            System.err.println("No native RingRTC support: ");
            ex.printStackTrace();
        }
    }
    

    public TringServiceImpl() {
        // no-op
    }

    public static TringService provider() {
        return instance;
    }
    
    public String getVersionInfo() {
        return "TringServiceImpl using "+libName;
    }

    public static long getNativeVersion() {
        return nativeVersion;
    }
    
    @Override
    public void setApi(io.privacyresearch.tringapi.TringApi api) {
        this.api = api;
        initiate();
    }

    private void initiate() {
        scope = MemorySession.openShared();
        tringlib_h.initRingRTC(toJString(scope, "Hello from Java"));
        this.callEndpoint = tringlib_h.createCallEndpoint(createStatusCallback(), 
                createAnswerCallback(), createOfferCallback(), createIceUpdateCallback(),
        createVideoFrameCallback());
        
    }
    
    private void processAudioInputs() {
        LOG.warning("Process Audio Inputs asked, not supported!");
        MemorySegment audioInputs = tringlib_h.getAudioInputs(scope, callEndpoint,0);
        MemorySegment name = TringDevice.name$slice(audioInputs);
        int namelen = (int)RString.len$get(name);
        MemoryAddress namebuff = RString.buff$get(name);
        MemorySegment ofAddress = MemorySegment.ofAddress(namebuff, namelen, scope);
        ByteBuffer bb = ofAddress.asByteBuffer();
        byte[] bname = new byte[namelen];
        bb.get(bname, 0, (int)namelen);
        String myname = new String(bname);
    }

    @Override
    public void receivedOffer(String peerId, long callId, int senderDeviceId, int receiverDeviceId,
            byte[] senderKey, byte[] receiverKey, byte[] opaque) {
        int mediaType = 0;
        long ageSec = 0;
        this.activeCallId = callId;
        LOG.info("Pass received offer to tringlib");
        tringlib_h.receivedOffer(callEndpoint, toJString(scope, peerId), callId, mediaType, senderDeviceId,
                receiverDeviceId, toJByteArray(scope, senderKey), toJByteArray(scope, receiverKey),
                toJByteArray(scope, opaque),
                ageSec);
    }

    @Override
    public void receivedAnswer(String peerId, long callId, int senderDeviceId,
            byte[] senderKey, byte[] receiverKey, byte[] opaque) {
        int mediaType = 0;
        long ageSec = 0;
        this.activeCallId = callId;
        LOG.info("Pass received answer to tringlib");
        tringlib_h.receivedAnswer(callEndpoint, toJString(scope, peerId), callId, senderDeviceId,
                toJByteArray(scope, senderKey), toJByteArray(scope, receiverKey),
                toJByteArray(scope, opaque));
    }

    public void setSelfUuid(String uuid) {
        tringlib_h.setSelfUuid(callEndpoint, toJString(scope, uuid));
    }

    @Override
    public void proceed(long callId, String iceUser, String icePwd, List<byte[]> ice) {
        MemorySegment icePack = toJByteArray2D(scope, ice);
        tringlib_h.setOutgoingAudioEnabled(callEndpoint, true);
        tringlib_h.proceedCall(callEndpoint, callId, BANDWIDTH_QUALITY_HIGH, 0,
                toJString(scope, iceUser), toJString(scope, icePwd), icePack);
    }

    @Override
    public void receivedIce(long callId, int senderDeviceId, List<byte[]> ice) {
        MemorySegment icePack = toJByteArray2D(scope, ice);
        tringlib_h.receivedIce(callEndpoint, callId, senderDeviceId, icePack);
    }

    @Override
    public void acceptCall() {
        LOG.info("Set audioInput to 0");
        tringlib_h.setAudioInput(callEndpoint, (short)0);
        LOG.info("Set audiorecording");
        tringlib_h.setOutgoingAudioEnabled(callEndpoint, true);
        LOG.info("And now accept the call");
        tringlib_h.acceptCall(callEndpoint, activeCallId);
        LOG.info("Accepted the call");
    }

    @Override
    public void ignoreCall() {
        LOG.info("Ignore the call");
        tringlib_h.ignoreCall(callEndpoint, activeCallId);
    }

    @Override
    public void hangupCall() {
        LOG.info("Hangup the call");
        tringlib_h.hangupCall(callEndpoint);
    }

    /**
     * start a call and return the call_id
     * @return the same call_id as the one we were passed, if success
     */
    @Override
    public long startOutgoingCall(long callId, String peerId, int localDeviceId, boolean enableVideo) {
        LOG.info("Tring will start outgoing call to "+peerId+" with localDevice "+localDeviceId+" and enableVideo = "+enableVideo);
        tringlib_h.setAudioInput(callEndpoint, (short)0);
        tringlib_h.setAudioOutput(callEndpoint, (short)0);
        tringlib_h.createOutgoingCall(callEndpoint, toJString(scope, peerId), enableVideo, localDeviceId, callId);
        return callId;
    }

    // for testing only
    public void setArray() {
        LOG.info("SET ARRAY");
        int CAP = 1000000;
        for (int i = 0; i < 1000; i++) {
            try (MemorySession rscope = MemorySession.openConfined()) {
                MemorySegment segment = MemorySegment.allocateNative(CAP, scope);
                tringlib_h.fillLargeArray(123, segment);
                ByteBuffer bb = segment.asByteBuffer();
                byte[] bar = new byte[CAP];
                bb.get(bar, 0, CAP);
                LOG.info("Got Array " + i + " sized " + bar.length);
            }
        }
        LOG.info("DONE");
    }

    @Override
    public TringFrame getRemoteVideoFrame(boolean skip) {
        int CAP = 5000000;
        try (MemorySession rscope = MemorySession.openShared()) {
            MemorySegment segment = MemorySegment.allocateNative(CAP, rscope);
            long res = tringlib_h.fillRemoteVideoFrame(callEndpoint, segment, CAP);
            if (res != 0) {
                int w = (int) (res >> 16);
                int h = (int) (res % (1 <<16));
                byte[] raw = new byte[w * h * 4];
                ByteBuffer bb = segment.asByteBuffer();
                bb.get(raw);
                TringFrame answer = new TringFrame(w, h, -1, raw);
                return answer;
            }
        } catch (Throwable t) {
            t.printStackTrace();
        }
        return null;
    }

    @Override
    public void enableOutgoingVideo(boolean enable) {
        tringlib_h.setOutgoingVideoEnabled(callEndpoint, enable);
    }

    @Override
    public void sendVideoFrame(int w, int h, int pixelFormat, byte[] raw) {
        try ( MemorySession session = MemorySession.openConfined()) {
            MemorySegment buff = session.allocateArray(ValueLayout.JAVA_BYTE, raw);
            tringlib_h.sendVideoFrame(callEndpoint, w, h, pixelFormat, buff);
        }
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
        public void apply(long id, long _x1, int direction, int type) {
            LOG.info("Got new status from ringrtc, id = " + id+", x1 = " + _x1+", dir = " + direction+", type = "+type);
            api.statusCallback(id, _x1, direction, type);
            sendAck();
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
    
    Addressable createOfferCallback() {
        OfferCallbackImpl sci = new OfferCallbackImpl();
        MemorySegment seg = createCallEndpoint$offerCallback.allocate(sci, scope);
        return seg.address();
    }

    class OfferCallbackImpl implements createCallEndpoint$offerCallback {
        @Override
        public void apply(MemorySegment opaque) {
            byte[] bytes = fromJArrayByte(scope, opaque);
            api.offerCallback(bytes);
            System.err.println("TRING, offer sent");
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
            byte[] bytes = fromJArrayByte(scope, icePack);
            List<byte[]> iceCandidates = new ArrayList<>();
            iceCandidates.add(bytes);

            api.iceUpdateCallback(iceCandidates);
            sendAck();
            LOG.info("iceUpdate done!");
        }
    }

    Addressable createVideoFrameCallback() {
        VideoFrameCallbackImpl sci = new VideoFrameCallbackImpl();
        MemorySegment seg = createCallEndpoint$videoFrameCallback.allocate(sci, scope);
        return seg.address();
    }
    
    @Deprecated
    class VideoFrameCallbackImpl implements createCallEndpoint$videoFrameCallback {
        @Override
        public void apply(MemoryAddress opaque, int w, int h, long size) {
            LOG.info("Got incoming video frame in Java layer, w = "+w+", h = " + h+", size = " + size);
            System.err.println("Opaque = " + opaque);
            MemorySegment segment = MemorySegment.ofAddress(opaque, size, scope);
            byte[] raw = segment.toArray(ValueLayout.JAVA_BYTE);
            synchronized (frameQueue) {
                LOG.info("Add frame to queue");
                frameQueue.add(new TringFrame(w,h,-1,raw));
                frameQueue.notifyAll();
            }
            LOG.info("Processed incoming video frame in Java layer");
            sendAck();
        }
    }

    // We need to inform ringrtc that we handled a message, so that it is ok
    // with sending the next message
    void sendAck() {
        MemorySegment callid = MemorySegment.allocateNative(8, scope);
        callid.set(ValueLayout.JAVA_LONG, 0l, activeCallId);
        tringlib_h.signalMessageSent(callEndpoint, callid);
    }

}
