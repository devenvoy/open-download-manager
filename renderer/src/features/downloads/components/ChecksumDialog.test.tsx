import { describe, expect, it, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ipc } from "@/lib/ipc";
import { ChecksumDialog } from "./ChecksumDialog";

vi.mock("@/lib/ipc", () => ({
  ipc: {
    verifyChecksum: vi.fn(),
    calculateChecksum: vi.fn(),
    readClipboard: vi.fn().mockResolvedValue(""),
  },
}));

describe("ChecksumDialog", () => {
  const defaultProps = {
    open: true,
    filePath: "/downloads/ubuntu-24.04.iso",
    fileName: "ubuntu-24.04.iso",
    onClose: vi.fn(),
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders when open and shows filename", () => {
    render(<ChecksumDialog {...defaultProps} />);
    expect(screen.getByText("Verify File Checksum")).toBeInTheDocument();
    expect(screen.getByText("ubuntu-24.04.iso")).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/Paste SHA-256/)).toBeInTheDocument();
  });

  it("detects SHA-256 algorithm on 64-character input", () => {
    render(<ChecksumDialog {...defaultProps} />);
    const input = screen.getByPlaceholderText(/Paste SHA-256/);
    const sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    fireEvent.change(input, { target: { value: sha256 } });
    expect(screen.getByText("SHA-256")).toBeInTheDocument();
  });

  it("detects CRC32 algorithm on 8-character input", () => {
    render(<ChecksumDialog {...defaultProps} />);
    const input = screen.getByPlaceholderText(/Paste SHA-256/);
    fireEvent.change(input, { target: { value: "1a2b3c4d" } });
    expect(screen.getByText("CRC32")).toBeInTheDocument();
  });

  it("calls ipc.verifyChecksum and displays match report", async () => {
    const mockReport = {
      matched: true,
      algorithm: "SHA-256",
      expectedHex: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      actualHex: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      fileBytes: 1048576,
    };
    vi.mocked(ipc.verifyChecksum).mockResolvedValueOnce(mockReport);

    render(<ChecksumDialog {...defaultProps} />);
    const input = screen.getByPlaceholderText(/Paste SHA-256/);
    fireEvent.change(input, { target: { value: mockReport.expectedHex } });

    const verifyBtn = screen.getByRole("button", { name: "Verify" });
    fireEvent.click(verifyBtn);

    await waitFor(() => {
      expect(ipc.verifyChecksum).toHaveBeenCalledWith(
        defaultProps.filePath,
        mockReport.expectedHex,
      );
      expect(screen.getByText(/Checksum Match: File is authentic/)).toBeInTheDocument();
    });
  });

  it("calls ipc.verifyChecksum and displays mismatch report", async () => {
    const mockReport = {
      matched: false,
      algorithm: "SHA-256",
      expectedHex: "0000000000000000000000000000000000000000000000000000000000000000",
      actualHex: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      fileBytes: 1048576,
    };
    vi.mocked(ipc.verifyChecksum).mockResolvedValueOnce(mockReport);

    render(<ChecksumDialog {...defaultProps} />);
    const input = screen.getByPlaceholderText(/Paste SHA-256/);
    fireEvent.change(input, { target: { value: mockReport.expectedHex } });

    const verifyBtn = screen.getByRole("button", { name: "Verify" });
    fireEvent.click(verifyBtn);

    await waitFor(() => {
      expect(screen.getByText(/Checksum Mismatch:/)).toBeInTheDocument();
      expect(screen.getByText("Copy actual")).toBeInTheDocument();
    });
  });
});
