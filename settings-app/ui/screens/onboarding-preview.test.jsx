import React from "react";
import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

let OnboardingPreviewMedia;

beforeAll(async () => {
  window.BlobUI = window.BlobUI || {};
  ({ OnboardingPreviewMedia } = await import("./shell.jsx"));
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  delete document.documentElement.dataset.reducedMotion;
});

describe("Onboarding preview media", () => {
  it("keeps the placeholder artwork as one active slide", () => {
    const { container } = render(<OnboardingPreviewMedia preview={{ key: "interface", media: [{ type: "artwork" }] }} />);

    expect(container.querySelectorAll(".onboarding-preview-slide").length).toBe(1);
    expect(container.querySelector(".onboarding-preview-slide.active")).not.toBeNull();
    expect(container.querySelector("svg.onboarding-preview-art")).not.toBeNull();
  });

  it("cycles multiple images with animated slides", () => {
    vi.useFakeTimers();
    const { container } = render(<OnboardingPreviewMedia preview={{
      key: "interface",
      media: [
        { type: "image", src: "./cdn/interface-one.png", alt: "Interface preview one" },
        { type: "image", src: "./cdn/interface-two.png", alt: "Interface preview two" },
      ],
    }} />);

    const slides = container.querySelectorAll(".onboarding-preview-slide");
    expect(slides.length).toBe(2);
    expect(slides[0].classList.contains("active")).toBe(true);
    expect(slides[1].classList.contains("active")).toBe(false);
    expect(slides[0].querySelector("img").getAttribute("src")).toBe("./cdn/interface-one.png");

    act(() => { vi.advanceTimersByTime(3200); });

    expect(slides[0].classList.contains("active")).toBe(false);
    expect(slides[1].classList.contains("active")).toBe(true);
    expect(slides[0].getAttribute("aria-hidden")).toBe("true");
    expect(slides[1].getAttribute("aria-hidden")).toBeNull();
  });

  it("uses one video instead of a slideshow when a video is provided", () => {
    const { container } = render(<OnboardingPreviewMedia preview={{
      key: "codex",
      media: [
        { type: "image", src: "./cdn/codex.png", alt: "Codex preview" },
        { type: "video", src: "./cdn/codex.mp4", alt: "Codex preview video" },
      ],
    }} />);

    const video = container.querySelector("video");
    expect(video).not.toBeNull();
    expect(video.getAttribute("src")).toBe("./cdn/codex.mp4");
    expect(container.querySelector(".onboarding-preview-slideshow")).toBeNull();
    expect(container.querySelectorAll(".onboarding-preview-slide").length).toBe(0);
  });
});
