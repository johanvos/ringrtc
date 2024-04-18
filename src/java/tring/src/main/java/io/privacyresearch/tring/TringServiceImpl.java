package io.privacyresearch.tring;

import io.privacyresearch.tringapi.PeekInfo;
import io.privacyresearch.tringapi.TringFrame;
import io.privacyresearch.tringapi.TringService;
import java.io.IOException;
import java.lang.foreign.Arena;
import java.lang.foreign.MemoryLayout;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.net.http.HttpResponse.BodyHandlers;
import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Map.Entry;
import java.util.UUID;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.logging.Level;
import java.util.logging.Logger;

public class TringServiceImpl implements TringService {

    static final int BANDWIDTH_QUALITY_HIGH = 2;
    private static final TringService instance = new TringServiceImpl();
    private static boolean nativeSupport = false;
    private static long nativeVersion = 0;

    private Arena scope;
    private long callEndpoint;
    private io.privacyresearch.tringapi.TringApi api;
    private long activeCallId;
    static String libName = "unknown";
    BlockingQueue<TringFrame> frameQueue = new LinkedBlockingQueue();

    // state for GroupCall, should be moved.
    private int clientId = -1;
    private byte[] localGroupId;
    
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
    
    @Override
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
        scope = Arena.ofShared();
        tringlib_h.initRingRTC(toJString(scope, "Hello from Java"));
        this.callEndpoint = tringlib_h.createCallEndpoint(createStatusCallback(), 
                createAnswerCallback(), createOfferCallback(),
                createIceUpdateCallback(),
                createGenericCallback(),
        createVideoFrameCallback());
        initializeNative(this.callEndpoint);
    }
    
    private void processAudioInputs() {
        LOG.warning("Process Audio Inputs asked, not supported!");
        MemorySegment audioInputs = tringlib_h.getAudioInputs(scope, callEndpoint,0);
        MemorySegment name = TringDevice.name(audioInputs);
        int namelen = (int)RString.len(name);
        MemorySegment namebuff = RString.buff(name);
//        MemorySegment ofAddress = MemorySegment.ofAddress(namebuff, namelen, scope);
//        ByteBuffer bb = ofAddress.asByteBuffer();
//        byte[] bname = new byte[namelen];
//        bb.get(bname, 0, (int)namelen);
//        String myname = new String(bname);
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
    public void receivedOpaqueMessage(byte[] senderUuid, int senderDeviceId,
            int localDeviceId, byte[] opaque, long age) {
        tringlib_h.receivedOpaqueMessage(callEndpoint, toJByteArray(scope, senderUuid),
            senderDeviceId, localDeviceId, toJByteArray(scope, opaque), age);
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
    public void proceed(long callId, String iceUser, String icePwd, String hostName, List<byte[]> ice) {
        MemorySegment icePack = toJByteArray2D(scope, ice);
        tringlib_h.setOutgoingAudioEnabled(callEndpoint, true);
        LOG.info("Proceeding call now...");
        tringlib_h.proceedCall(callEndpoint, callId, BANDWIDTH_QUALITY_HIGH, 0,
                toJString(scope, iceUser), toJString(scope, icePwd), toJString(scope, hostName), icePack);
        LOG.info("Proceeded call");
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
        if (clientId < 0) {
            tringlib_h.hangupCall(callEndpoint);
        } else {
            tringlib_h.disconnect(callEndpoint, clientId);
        }
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

    @Override
    public void peekGroupCall(byte[] membershipProof, byte[] members) {
        LOG.info("Need to peek groupcall, memberslength = "+members.length);
        tringlib_h.peekGroupCall(callEndpoint, toJByteArray(scope, membershipProof)
        , toJByteArray(scope, members));
    }

    // see Android IncomingGroupCallActionProcessor.handleAcceptCall
    @Override
    public long createGroupCallClient(byte[] nogroupId, String sfu, byte[] hkdf) {
        LOG.info("delegate creategroupcallclient to rust, groupId = "+Arrays.toString(localGroupId));
        long myclientId = tringlib_h.createGroupCallClient(callEndpoint, toJByteArray(scope, localGroupId),
                toJString(scope, sfu), toJByteArray(scope, hkdf));
        this.clientId = (int)myclientId;
        LOG.info("Created client, id = "+clientId+". Will connect now");
        tringlib_h.setOutgoingAudioMuted(callEndpoint, (int)clientId, true);
        tringlib_h.setOutgoingVideoMuted(callEndpoint, (int)clientId, true);
        setGroupBandWidth((int)clientId, 2); // 2 = NORMAL
        tringlib_h.group_connect(callEndpoint, (int)clientId);
        LOG.info("Connected, id = "+clientId);
        LOG.info("Ask for video");
        requestVideo(callEndpoint, (int)clientId, 1);
        LOG.info("Asked for video");
        tringlib_h.setOutgoingAudioMuted(callEndpoint, (int)clientId, false);
        tringlib_h.setOutgoingVideoMuted(callEndpoint, (int)clientId, false);
        setGroupBandWidth((int)clientId, 2); // 2 = NORMAL
        tringlib_h.join(callEndpoint, (int)clientId);
        return clientId;
    }

    @Override
    public void setGroupBandWidth(int groupId, int bandwidthMode) {
        tringlib_h.setDataMode(callEndpoint, groupId, bandwidthMode);
    }

    // for testing only
    public void setArray() {
        LOG.info("SET ARRAY");
        int CAP = 1000000;
        for (int i = 0; i < 1000; i++) {
            try (Arena rscope = Arena.ofShared()) {
                MemorySegment segment = rscope.allocate(CAP);
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
        try (Arena rscope = Arena.ofShared()) {
            MemorySegment segment = rscope.allocate(CAP);
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
        try ( Arena session = Arena.ofConfined()) {
            int size = raw.length;
            MemorySegment rawSegment = MemorySegment.ofArray(raw);
            MemorySegment buff = session.allocate(MemoryLayout.sequenceLayout(size, ValueLayout.JAVA_BYTE));
            buff.copyFrom(rawSegment);
            tringlib_h.sendVideoFrame(callEndpoint, w, h, pixelFormat, buff);
        }
    }
    
    static MemorySegment toJByteArray2D(Arena ms, List<byte[]> rows) {
        LOG.info("Create JB2 with "+rows.size()+ " rows: ");
        MemorySegment answer = JByteArray2D.allocate(ms);
        JByteArray2D.len(answer, rows.size());
        MemorySegment bufferSegment = JByteArray2D.buff(answer);
        LOG.info("Prep JB2D, length = " + JByteArray2D.len(answer));

        for (int i = 0; i < rows.size(); i++) {
            MemorySegment singleRowSegment = toJByteArray(ms, rows.get(i));
            JByteArray2D.buff(bufferSegment, i, singleRowSegment);

        }
        LOG.info("Size of memory segment = " + answer.byteSize());
        LOG.info("Return JB2D, length = " + JByteArray2D.len(answer));
        return answer;
    }

    static MemorySegment toJByteArray(Arena arena, byte[] raw) {
        LOG.info("Create JB with "+raw.length+" bytes");
        MemorySegment answer = JByteArray.allocate(arena);
        int size = raw.length;
        MemorySegment rawSegment = MemorySegment.ofArray(raw);
        MemorySegment transfer = arena.allocate(size);
        transfer.copyFrom(rawSegment);
        JByteArray.len(answer, size);
        JByteArray.buff(answer, transfer);
        LOG.info("Size of JB = "+answer.byteSize());
        return answer;
    }
    
    static byte[] fromJArrayByte(MemorySegment jArrayByte) {
        int len = (int)JArrayByte.len(jArrayByte);
        MemorySegment dataSegment = JArrayByte.data(jArrayByte).asSlice(0, len);
        byte[] destArr = new byte[len];
        MemorySegment dstSeq = MemorySegment.ofArray(destArr);
        dstSeq.copyFrom(dataSegment);
        return destArr;
    }

    static MemorySegment toJString(Arena arena, String src) {
        MemorySegment answer = JPString.allocate(arena);
        byte[] bytes = src.getBytes();
        JPString.len(answer, bytes.length);
        MemorySegment byteBuffer = MemorySegment.ofArray(bytes);
        MemorySegment pass = arena.allocate(bytes.length);
        pass.copyFrom(byteBuffer);
        JPString.buff(answer, pass);
        return answer;
    }

    private List<UUID> getUUIDs(List joined) {
        List<UUID> joinedMembers = new ArrayList<>();
        for (Object entry : joined) {
            ByteBuffer bb = ByteBuffer.wrap((byte[]) entry);
            joinedMembers.add(new UUID(bb.getLong(), bb.getLong()));
        }
        return joinedMembers;
    }

    public void handlePeekChanged(List joined, byte[] creator, String era, long maxDevices, long deviceCount) {
        LOG.info("In java: GOT PEEK CHANGED");
        List<UUID> joinedMembers = getUUIDs(joined);
        UUID creatorId = null;
        if (creator != null) {
            ByteBuffer bb = ByteBuffer.wrap(creator);
            creatorId = new UUID(bb.getLong(), bb.getLong());
        }
        LOG.info("Joined: " + joinedMembers);
        LOG.info("Creator: " + creatorId);
        PeekInfo peekInfo = new PeekInfo(joinedMembers, creatorId, era, maxDevices, deviceCount);
    }

    public void handlePeekResponse(List joined, byte[] creator, String era, long maxDevices, long deviceCount) {
        LOG.info("JAVA: GOT PEEK RESULT");
        List<UUID> joinedMembers = getUUIDs(joined);
        ByteBuffer bb = ByteBuffer.wrap(creator);
        UUID creatorId = new UUID(bb.getLong(), bb.getLong());
        PeekInfo peekInfo = new PeekInfo(joinedMembers, creatorId,era, maxDevices, deviceCount);
        api.receivedGroupCallPeekForRingingCheck(peekInfo);
    }

    public void handleRemoteDevicesChanged(List devices) {
        LOG.info("Devices changed into "+devices);
        for (Object entry : devices) {
            ByteBuffer bb = ByteBuffer.wrap((byte[]) entry);
            int demuxId = bb.getInt();
            LOG.info("Schedule call to request video from "+demuxId);
            Runnable r = () -> requestVideo(callEndpoint, clientId, demuxId);
            executeRequest(r);
        }
    }

    public void makeHttpRequest(String uri, byte m, int reqid, byte[] headers, byte[] body) {
        try {
            LOG.info("MAKE REQUEST:" + uri + " and method = " + m + ", reqid = " + reqid + "and body has size " + body.length);
            ByteBuffer bb = ByteBuffer.wrap(headers);
            Map<String, String> headerMap = new HashMap<>();
            while (bb.hasRemaining()) {
                byte[] b = new byte[bb.getInt()];
                bb.get(b);
                String key = new String(b);
                b = new byte[bb.getInt()];
                bb.get(b);
                String val = new String(b);
                headerMap.put(key, val);
            }
            HttpClient client = HttpClient.newHttpClient();
            HttpRequest.Builder builder = HttpRequest.newBuilder()
                    .uri(URI.create(uri));
            for (Entry<String, String> entry : headerMap.entrySet()) {
                builder.header(entry.getKey(), entry.getValue());
            }
            if (m == 0x1) { // PUT
                ByteBuffer bodybb = ByteBuffer.wrap(body);
                long bs = bodybb.getLong();
                byte[] bd = new byte[(int) bs];
                bodybb.get(bd);
                LOG.info("We need to PUT");
                builder.PUT(HttpRequest.BodyPublishers.ofByteArray(bd));
            }
            HttpRequest request = builder.build();

            HttpResponse<String> response;
            try {
                response = client.send(request, BodyHandlers.ofString());
                tringlib_h.panamaReceivedHttpResponse(callEndpoint, reqid, response.statusCode(), toJByteArray(scope, response.body().getBytes()));
            } catch (IOException | InterruptedException ex) {
               LOG.log(Level.SEVERE, null, ex);
            }
        } catch (Throwable t) {
            LOG.severe("Whoops! " + t);
            t.printStackTrace();
        }
    }
//
//    public static void nomakeStaticHttpRequest(String request) {
//        System.err.println("MAKE STATIC REQUEST: request");
//    }  
//
    private native void initializeNative(long callEndpoint);
//    private native void ringrtcReceivedHttpResponse(long callEndpoint, long requestid, int status, byte[] body);
//
    private native void requestVideo(long callEndpoint, int clientid, int demuxid);
//
    MemorySegment createStatusCallback() {
        StatusCallbackImpl sci = new StatusCallbackImpl();
        MemorySegment seg = createCallEndpoint$statusCallback.allocate(sci, scope);
        return seg;
    }



    class StatusCallbackImpl implements createCallEndpoint$statusCallback.Function {
        @Override
        public void apply(long id, long _x1, int direction, int type) {
            LOG.info("Got new status from ringrtc, id = " + id+", x1 = " + _x1+", dir = " + direction+", type = "+type);
            api.statusCallback(id, _x1, direction, type);
            sendAck();
        }
    }
    
    MemorySegment createAnswerCallback() {
        AnswerCallbackImpl sci = new AnswerCallbackImpl();
        MemorySegment seg = createCallEndpoint$answerCallback.allocate(sci, scope);
        return seg;
    }
    
    class AnswerCallbackImpl implements createCallEndpoint$answerCallback.Function {
        @Override
        public void apply(MemorySegment opaque) {
            System.err.println("TRINGBRIDGE, send answer!");
            byte[] bytes = fromJArrayByte(opaque);
            System.err.println("TRING, bytes to send = "+java.util.Arrays.toString(bytes));
            api.answerCallback(bytes);
            System.err.println("TRING, answer sent");
            sendAck();
            System.err.println("TRING, ack sent");
        }
    }
    
    MemorySegment createOfferCallback() {
        OfferCallbackImpl sci = new OfferCallbackImpl();
        MemorySegment seg = createCallEndpoint$offerCallback.allocate(sci, scope);
        return seg;
    }

    class OfferCallbackImpl implements createCallEndpoint$offerCallback.Function {
        @Override
        public void apply(MemorySegment opaque) {
            byte[] bytes = fromJArrayByte(opaque);
            api.offerCallback(bytes);
            System.err.println("TRING, offer sent");
            sendAck();
            System.err.println("TRING, ack sent");
        }
    }

    MemorySegment createIceUpdateCallback() {
        IceUpdateCallbackImpl sci = new IceUpdateCallbackImpl();
        MemorySegment seg = createCallEndpoint$iceUpdateCallback.allocate(sci, scope);
        return seg;
    }

    class IceUpdateCallbackImpl implements createCallEndpoint$iceUpdateCallback.Function {

        @Override
        public void apply(MemorySegment icePack) {
            byte[] bytes = fromJArrayByte(icePack);
            List<byte[]> iceCandidates = new ArrayList<>();
            iceCandidates.add(bytes);

            api.iceUpdateCallback(iceCandidates);
            sendAck();
            LOG.info("iceUpdate done!");
        }
    }
    MemorySegment createGenericCallback() {
        GenericCallbackImpl sci = new GenericCallbackImpl();
        MemorySegment seg = createCallEndpoint$genericCallback.allocate(sci, scope);
        return seg;
    }

    class GenericCallbackImpl implements createCallEndpoint$genericCallback.Function {

        @Override
        public void apply(int opcode, MemorySegment data) {
            byte[] bytes = fromJArrayByte(data);
            LOG.info("Got generic  callback, opcode = " + opcode + " and data = " + Arrays.toString(bytes));
            if (opcode == 1) {
                LOG.info("This will lead to a groupCallUpdateRing");
                ByteBuffer bb = ByteBuffer.wrap(bytes);
                int groupIdLen = bb.getInt();
                byte[] groupId = new byte[groupIdLen];
                bb.get(groupId);
                TringServiceImpl.this.localGroupId = groupId;
                long ringId = bb.getLong();
                byte[] senderBytes = new byte[bb.remaining() - 1];
                bb.get(senderBytes);
                byte status = bb.get();
                api.groupCallUpdateRing(groupId, ringId, senderBytes, status);
            }
            if (opcode == 2) {
                // connectionStateChange
                ByteBuffer bb = ByteBuffer.wrap(bytes);
                int clientId = bb.getInt();
                int connectionStatus = bb.getInt();
                LOG.info("ConnectionState for " + clientId + " changed to " + connectionStatus);
            }
            if (opcode == 3) {
                // requestMembershipProof
                LOG.info("Handling requestMembershipProof");
                ByteBuffer bb = ByteBuffer.wrap(bytes);
                int clientId = bb.getInt();
                Runnable r = () -> {
                    byte[] token = api.requestGroupMembershipToken(TringServiceImpl.this.localGroupId);
                    tringlib_h.setMembershipProof(callEndpoint, clientId, toJByteArray(scope, token));
                };
                executeRequest(r);
                LOG.info("Handled requestMembershipProof");
            }
            if (opcode == 4) {
                // requestGroupMembers
                ByteBuffer bb = ByteBuffer.wrap(bytes);
                int clientId = bb.getInt();
                byte[] memberinfo = api.requestGroupMemberInfo(TringServiceImpl.this.localGroupId);
                tringlib_h.setGroupMembers(callEndpoint, clientId, toJByteArray(scope, memberinfo));
            }
            if (opcode == 5) {
                // sendCallMessage
                ByteBuffer bb = ByteBuffer.wrap(bytes);
                UUID recipient = new UUID(bb.getLong(), bb.getLong());
                int mlen = bb.getInt();
                byte[] messageb = new byte[mlen];
                LOG.info("Will send opaquecallmessage to " + recipient+" with byte len = " + mlen);
                bb.get(messageb);
                api.sendOpaqueCallMessage(recipient, messageb, 0);
            }
        }

    }

    MemorySegment createVideoFrameCallback() {
        VideoFrameCallbackImpl sci = new VideoFrameCallbackImpl();
        MemorySegment seg = createCallEndpoint$videoFrameCallback.allocate(sci, scope);
        return seg;
    }
    
    @Deprecated
    class VideoFrameCallbackImpl implements createCallEndpoint$videoFrameCallback.Function {
        @Override
        public void apply(MemorySegment opaque, int w, int h, long size) {
            LOG.info("Got incoming video frame in Java layer, w = "+w+", h = " + h+", size = " + size);
            System.err.println("Opaque = " + opaque);
            MemorySegment segment = opaque.asSlice(0, size);//MemorySegment.ofAddress(opaque, size, scope);
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
        LOG.info("Send Ack");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment callid = arena.allocateFrom(ValueLayout.JAVA_LONG, activeCallId);
            tringlib_h.signalMessageSent(callEndpoint, callid);
        }
        LOG.info("Send Ack done");
    }

    ExecutorService executor = Executors.newFixedThreadPool(1);

    private void executeRequest(Runnable r) {
        LOG.info("Executing request "+r);
        Future<?> submit = executor.submit(r);
        LOG.info("Execution state = " + submit.state());
    }
}
