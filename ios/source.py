"""Writes the SideStore / AltStore source for one build (see ios/README.md).

usage: source.py <ipa> <version> <build code> <tag> <owner/repo>

Lists only this build; SideStore offers it as the update.
"""
import datetime
import json
import os
import sys

ipa, version, build, tag, repo = sys.argv[1:6]
base = f"https://github.com/{repo}/releases"
beta = version.count(".") >= 2
print(json.dumps({
    "name": "Stream Sound",
    "identifier": "app.streamsound.source",
    "subtitle": "เสียงจากเครื่องหนึ่งไปอีกเครื่องใน Wi-Fi เดียวกัน",
    "sourceURL": f"{base}/latest/download/StreamSound-source.json",
    "iconURL": f"{base}/latest/download/StreamSound-icon.png",
    "tintColor": "#E62639",
    "apps": [{
        "name": "Stream Sound",
        "bundleIdentifier": "app.streamsound.ios",
        "developerName": "Stream Sound",
        "subtitle": "รับส่งเสียงข้ามเครื่องแบบหน่วงต่ำ",
        "localizedDescription": "รับเสียงจาก PC / Android มาเล่นบน iPhone และส่งเสียงทั้งเครื่องของ iPhone ไปเครื่องอื่นใน Wi-Fi เดียวกัน",
        "iconURL": f"{base}/latest/download/StreamSound-icon.png",
        "tintColor": "#E62639",
        "category": "utilities",
        "versions": [{
            "version": version,
            "buildVersion": build,
            "date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "localizedDescription": f"Stream Sound v{version}" + (" (เบต้า)" if beta else ""),
            "downloadURL": f"{base}/download/{tag}/StreamSound.ipa",
            "size": os.path.getsize(ipa),
            "minOSVersion": "15.0",
        }],
        "appPermissions": {
            "entitlements": [],
            "privacy": {
                "NSLocalNetworkUsageDescription": "ค้นหาเครื่องอื่นและรับส่งเสียงใน Wi-Fi",
                "NSMicrophoneUsageDescription": "ส่งเสียงจากไมโครโฟน",
            },
        },
    }],
    "news": [],
}, ensure_ascii=False, indent=2))
