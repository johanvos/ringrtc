package io.privacyresearch.tringapi;

import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Optional;
import java.util.ServiceLoader;
import java.util.logging.Logger;
import java.util.stream.Collectors;

 /**
  * This class provides the access points for the application to interact with
  * RingRTC. 
  * Methods here are invoked by Equation.
  * When RingRTC wants to call back into the application, the TringApi interface
  * is used.
  * @author johan
  */
public class TringBridge {
    
    private TringService service;
    private static final Logger LOG = Logger.getLogger(TringBridge.class.getName());

    public TringBridge(final TringApi api) {
        ServiceLoader<TringService> loader = ServiceLoader.load(TringService.class);
        Optional<TringService> serviceOpt = loader.findFirst();
        serviceOpt.ifPresentOrElse(s -> {
            this.service = s;
            this.service.setApi(api);
        }, () -> {
            LOG.warning("No tring service!");
        });

    }

    public String getVersionInfo() {
        if (service == null) {
            return "No TringService registered";
        } else {
            return service.getVersionInfo();
        }
    }

    public void acceptCall() {
        service.acceptCall();
    }

    public void ignoreCall() {
        service.ignoreCall();
    }

    public void hangupCall() {
        service.hangupCall();
    }

    public void proceed(long callId, String iceUser, String icePassword, List<String> ice) {
        List<byte[]> iceb = ice.stream().map(s -> s.getBytes(StandardCharsets.UTF_8)).collect(Collectors.toList());
        service.proceed(callId, iceUser, icePassword, iceb);
    }

    public void receivedIce(long callId, int senderDeviceId, List<byte[]> ice) {
        service.receivedIce(callId, senderDeviceId, ice);
    }

    public void receivedOffer(String peerId, long callId, int senderDeviceId, int receiverDeviceId,
            byte[] senderKey, byte[] receiverKey, byte[] opaque) {
        service.receivedOffer(peerId, callId, senderDeviceId, receiverDeviceId, senderKey, receiverKey, opaque);
    }

    public void receivedAnswer(String peerId, long callId, int receiverDeviceId,
            byte[] senderKey, byte[] receiverKey, byte[] opaque) {
        service.receivedAnswer(peerId, callId, receiverDeviceId, senderKey, receiverKey, opaque);
    }

    public long startOutgoingCall(long callId, String peerId, int localDeviceId, boolean enableVideo) {
        return service.startOutgoingCall(callId, peerId, localDeviceId, enableVideo);
    }

    public void enableOutgoingVideo(boolean enable) {
        service.enableOutgoingVideo(enable);
    }

    public TringFrame getRemoteVideoFrame() {
        return service.getRemoteVideoFrame();
    }  

    public void sendVideoFrame(int width, int height, int pixelFormat, byte[] raw) {
        service.sendVideoFrame(width, height, pixelFormat, raw);
    }

    public void setArray() {
        service.setArray();
    }
}
