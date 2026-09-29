#!/usr/bin/env swift
// Render a Matter setup payload locally using macOS frameworks. No network access.
import CoreImage
import Darwin
import Foundation
import ImageIO
import UniformTypeIdentifiers

enum RenderError: Error, CustomStringConvertible {
    case message(String)
    var description: String {
        switch self { case .message(let text): return text }
    }
}

func render() throws {
    guard CommandLine.arguments.count == 2 else {
        throw RenderError.message("Usage: commissioning qr | swift scripts/pairing-qr.swift OUTPUT.png")
    }
    let input = String(data: FileHandle.standardInput.readDataToEndOfFile(), encoding: .utf8)?
        .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    let prefix = "KR OK qr_payload="
    let payload = input.hasPrefix(prefix) ? String(input.dropFirst(prefix.count)) : input
    let base38 = CharacterSet(charactersIn: "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-.")
    guard payload.hasPrefix("MT:"), payload.count > 3, payload.count <= 128,
        payload.dropFirst(3).unicodeScalars.allSatisfy({ base38.contains($0) }) else {
        throw RenderError.message("Expected one MT: payload or successful commissioning qr response on stdin")
    }
    guard let filter = CIFilter(name: "CIQRCodeGenerator") else {
        throw RenderError.message("macOS QR encoder unavailable")
    }
    filter.setValue(Data(payload.utf8), forKey: "inputMessage")
    filter.setValue("M", forKey: "inputCorrectionLevel")
    guard let qr = filter.outputImage else {
        throw RenderError.message("QR encoding failed")
    }
    // Four white modules on every side and integer scaling preserve scanability.
    let bounds = qr.extent.insetBy(dx: -4, dy: -4)
    let white = CIImage(color: .white).cropped(to: bounds)
    let bordered = qr.composited(over: white)
        .transformed(by: CGAffineTransform(scaleX: 10, y: 10))
    guard let image = CIContext().createCGImage(bordered, from: bordered.extent) else {
        throw RenderError.message("QR rasterization failed")
    }
    let data = NSMutableData()
    guard let destination = CGImageDestinationCreateWithData(data, UTType.png.identifier as CFString, 1, nil) else {
        throw RenderError.message("PNG encoder unavailable")
    }
    CGImageDestinationAddImage(destination, image, nil)
    guard CGImageDestinationFinalize(destination) else {
        throw RenderError.message("PNG encoding failed")
    }
    let path = CommandLine.arguments[1]
    let descriptor = Darwin.open(path, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW, S_IRUSR | S_IWUSR)
    guard descriptor >= 0 else {
        throw RenderError.message("Could not create a new private PNG: \(String(cString: strerror(errno)))")
    }
    let file = FileHandle(fileDescriptor: descriptor, closeOnDealloc: true)
    defer { try? file.close() }
    try file.write(contentsOf: data as Data)
    print("Saved local pairing QR to \(path)")
}

do {
    try render()
} catch {
    FileHandle.standardError.write(Data("\(error)\n".utf8))
    exit(1)
}
