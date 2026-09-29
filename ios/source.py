"""Writes the SideStore / AltStore source for one build (see ios/README.md).

usage: source.py <ipa> <build> <owner/repo>
"""
import datetime
import json
import os
import sys

ipa, build, repo = sys.argv[1], sys.argv[2], sys.argv[3]
base = f"https://github.com/{repo}/releases"
version = f"0.1.{build}"
print(json.dumps({
    "name": "Stream Sound",
    "identifier": "app.streamsound.source",
    "subtitle": "เสียงจากเครื่องหนึ่งไปอีกเครื่องใน Wi-Fi เดียวกัน",
    "sourceURL": f"{base}/latest/download/StreamSound-source.json",
    "iconURL": f"https://raw.githubusercontent.com/{repo}/main/ios/App/Assets.xcassets/AppIcon.appiconset/icon-1024.png",
    "tintColor": "#E62639",
    "apps": [{
        "name": "Stream Sound",
        "bundleIdentifier": "app.streamsound.ios",
        "developerName": "Stream Sound",
        "subtitle": "รับส่งเสียงข้ามเครื่องแบบหน่วงต่ำ",
        "localizedDescription": "รับเสียงจาก PC / Android มาเล่นบน iPhone และส่งเสียงทั้งเครื่องของ iPhone ไปเครื่องอื่นใน Wi-Fi เดียวกัน",
        "iconURL": f"https://raw.githubusercontent.com/{repo}/main/ios/App/Assets.xcassets/AppIcon.appiconset/icon-1024.png",
        "tintColor": "#E62639",
        "category": "utilities",
        "versions": [{
            "version": version,
            "buildVersion": build,
            "date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "localizedDescription": f"Stream Sound build {build}",
            "downloadURL": f"{base}/download/v{version}/StreamSound.ipa",
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
