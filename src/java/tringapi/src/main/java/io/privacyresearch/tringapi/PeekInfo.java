package io.privacyresearch.tringapi;

import java.util.Collection;
import java.util.UUID;

/**
 *
 * @author johan
 */
public class PeekInfo {

    private final Collection<UUID> joinedMembers;
    private final UUID creator;
    private final String eraId;
    private final Long maxDevices;

    private final long deviceCount;

    public PeekInfo(
            Collection<UUID> joinedMembers,
            UUID creator,
            String eraId,
            Long maxDevices,
            long deviceCount
    ) {
        this.joinedMembers = joinedMembers;
        this.creator = creator;
        this.eraId = eraId;
        this.maxDevices = maxDevices;
        this.deviceCount = deviceCount;
    }

    public Collection<UUID> getJoinedMembers() {
        return joinedMembers;
    }

    public UUID getCreator() {
        return creator;
    }

    public String getEraId() {
        return eraId;
    }

    public Long getMaxDevices() {
        return maxDevices;
    }

    public long getDeviceCount() {
        return deviceCount;
    }

}
