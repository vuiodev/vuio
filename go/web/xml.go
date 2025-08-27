package web

import (
	"encoding/xml"
	"fmt"
	"log/slog"
	"strconv"
	"strings"

	"vuio-go/database"
	"vuio-go/platform"
	"vuio-go/state"
)

// xmlEscape escapes characters for XML.
func xmlEscape(s string) string {
	var sb strings.Builder
	xml.EscapeText(&sb, []byte(s))
	return sb.String()
}

func generateDescriptionXML(state *state.AppState) string {
	cfg := state.GetConfig()
	return fmt.Sprintf(
		`<?xml version="1.0" encoding="UTF-8"?>
<root xmlns="urn:schemas-upnp-org:device-1-0">
    <specVersion><major>1</major><minor>0</minor></specVersion>
    <device>
        <deviceType>urn:schemas-upnp-org:device:MediaServer:1</deviceType>
        <friendlyName>%s</friendlyName>
        <manufacturer>VuIO-Go</manufacturer>
        <modelName>VuIO-Go Server</modelName>
        <UDN>uuid:%s</UDN>
        <serviceList>
            <service>
                <serviceType>urn:schemas-upnp-org:service:ContentDirectory:1</serviceType>
                <serviceId>urn:upnp-org:serviceId:ContentDirectory</serviceId>
                <SCPDURL>/ContentDirectory.xml</SCPDURL>
                <controlURL>/control/ContentDirectory</controlURL>
                <eventSubURL>/event/ContentDirectory</eventSubURL>
            </service>
        </serviceList>
    </device>
</root>`,
		xmlEscape(cfg.Server.Name),
		cfg.Server.UUID,
	)
}

func generateSCPDXML() string {
	return `<?xml version="1.0" encoding="UTF-8"?>
<scpd xmlns="urn:schemas-upnp-org:service-1-0">
    <specVersion><major>1</major><minor>0</minor></specVersion>
    <actionList>
        <action>
            <name>Browse</name>
            <argumentList>
                <argument><name>ObjectID</name><direction>in</direction><relatedStateVariable>A_ARG_TYPE_ObjectID</relatedStateVariable></argument>
                <argument><name>BrowseFlag</name><direction>in</direction><relatedStateVariable>A_ARG_TYPE_BrowseFlag</relatedStateVariable></argument>
                <argument><name>Filter</name><direction>in</direction><relatedStateVariable>A_ARG_TYPE_Filter</relatedStateVariable></argument>
                <argument><name>StartingIndex</name><direction>in</direction><relatedStateVariable>A_ARG_TYPE_Index</relatedStateVariable></argument>
                <argument><name>RequestedCount</name><direction>in</direction><relatedStateVariable>A_ARG_TYPE_Count</relatedStateVariable></argument>
                <argument><name>SortCriteria</name><direction>in</direction><relatedStateVariable>A_ARG_TYPE_SortCriteria</relatedStateVariable></argument>
                <argument><name>Result</name><direction>out</direction><relatedStateVariable>A_ARG_TYPE_Result</relatedStateVariable></argument>
                <argument><name>NumberReturned</name><direction>out</direction><relatedStateVariable>A_ARG_TYPE_Count</relatedStateVariable></argument>
                <argument><name>TotalMatches</name><direction>out</direction><relatedStateVariable>A_ARG_TYPE_Count</relatedStateVariable></argument>
                <argument><name>UpdateID</name><direction>out</direction><relatedStateVariable>A_ARG_TYPE_UpdateID</relatedStateVariable></argument>
            </argumentList>
        </action>
    </actionList>
    <serviceStateTable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_ObjectID</name><dataType>string</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_BrowseFlag</name><dataType>string</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_Filter</name><dataType>string</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_Index</name><dataType>ui4</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_Count</name><dataType>ui4</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_SortCriteria</name><dataType>string</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_Result</name><dataType>string</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_UpdateID</name><dataType>ui4</dataType></stateVariable>
        <stateVariable sendEvents="yes"><name>SystemUpdateID</name><dataType>ui4</dataType></stateVariable>
        <stateVariable sendEvents="yes"><name>ContainerUpdateIDs</name><dataType>string</dataType></stateVariable>
    </serviceStateTable>
</scpd>`
}

type BrowseParams struct {
	ObjectID       string
	StartingIndex  int
	RequestedCount int
}

func parseBrowseParams(actionXML string) BrowseParams {
	return BrowseParams{
		ObjectID:       getXMLValue(actionXML, "ObjectID"),
		StartingIndex:  getXMLValueInt(actionXML, "StartingIndex"),
		RequestedCount: getXMLValueInt(actionXML, "RequestedCount"),
	}
}

func generateBrowseResponse(objectID string, subdirs []database.MediaDirectory, files []database.MediaFile, totalMatches int, state *state.AppState) string {
	var didl strings.Builder
	didl.WriteString(`<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/">`)

	if objectID == "0" {
		// Root containers
		didl.WriteString(`<container id="video" parentID="0" restricted="1"><dc:title>Video</dc:title><upnp:class>object.container</upnp:class></container>`)
		didl.WriteString(`<container id="audio" parentID="0" restricted="1"><dc:title>Audio</dc:title><upnp:class>object.container</upnp:class></container>`)
		didl.WriteString(`<container id="image" parentID="0" restricted="1"><dc:title>Image</dc:title><upnp:class>object.container</upnp:class></container>`)
	} else {
		// Use the subdirs and files passed from the handler
		serverIP, err := platform.GetPrimaryIP()
		if err != nil {
			slog.Error("Could not get primary IP for browse response", "error", err)
			serverIP = "127.0.0.1" // Fallback
		}
		port := state.GetConfig().Server.Port

		for _, dir := range subdirs {
			// Ensure containerID is correctly formed, especially for nested paths.
			// Trim trailing slash from objectID to prevent double slashes like "video//subdir"
			containerID := fmt.Sprintf("%s/%s", strings.TrimRight(objectID, "/"), dir.Name)
			didl.WriteString(fmt.Sprintf(`<container id="%s" parentID="%s" restricted="1"><dc:title>%s</dc:title><upnp:class>object.container</upnp:class></container>`,
				xmlEscape(containerID), xmlEscape(objectID), xmlEscape(dir.Name)))
		}
		for _, file := range files {
			url := fmt.Sprintf("http://%s:%d/media/%d", serverIP, port, file.ID)
			didl.WriteString(fmt.Sprintf(`<item id="%d" parentID="%s" restricted="1"><dc:title>%s</dc:title><upnp:class>%s</upnp:class><res protocolInfo="http-get:*:%s:*" size="%d">%s</res></item>`,
				file.ID, xmlEscape(objectID), xmlEscape(file.Filename), getUPnPClass(file.MimeType), file.MimeType, file.Size, xmlEscape(url)))
		}
	}

	didl.WriteString(`</DIDL-Lite>`)

	numberReturned := len(subdirs) + len(files)
	if objectID == "0" {
		numberReturned = 3
	}
	if totalMatches == 0 {
		totalMatches = numberReturned
	}

	return fmt.Sprintf(`<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
    <s:Body>
        <u:BrowseResponse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
            <Result>%s</Result>
            <NumberReturned>%d</NumberReturned>
            <TotalMatches>%d</TotalMatches>
            <UpdateID>%d</UpdateID>
        </u:BrowseResponse>
    </s:Body>
</s:Envelope>`,
		xmlEscape(didl.String()),
		numberReturned,
		totalMatches,
		state.GetUpdateID())
}

func getUPnPClass(mimeType string) string {
	switch {
	case strings.HasPrefix(mimeType, "video/"):
		return "object.item.videoItem"
	case strings.HasPrefix(mimeType, "audio/"):
		return "object.item.audioItem.musicTrack"
	case strings.HasPrefix(mimeType, "image/"):
		return "object.item.imageItem.photo"
	default:
		return "object.item"
	}
}

// --- XML Parsing Helpers ---

func getXMLValue(xmlStr, tagName string) string {
	startTag := "<" + tagName + ">"
	endTag := "</" + tagName + ">"
	start := strings.Index(xmlStr, startTag)
	if start == -1 {
		return ""
	}
	start += len(startTag)
	end := strings.Index(xmlStr[start:], endTag)
	if end == -1 {
		return ""
	}
	return xmlStr[start : start+end]
}

func getXMLValueInt(xmlStr, tagName string) int {
	valStr := getXMLValue(xmlStr, tagName)
	if valStr == "" {
		return 0
	}
	val, _ := strconv.Atoi(valStr)
	return val
}

func getSOAPAction(body, actionName string) (string, bool) {
	tag := fmt.Sprintf("<u:%s", actionName)
	if idx := strings.Index(body, tag); idx != -1 {
		return body[idx:], true
	}
	return "", false
}
