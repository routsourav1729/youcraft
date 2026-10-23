from typing import Literal
from uuid import uuid4
from pydantic import BaseModel, ConfigDict, Field, model_validator


def uid():
    return uuid4().hex


class Model(BaseModel):
    model_config = ConfigDict(extra="forbid", allow_inf_nan=False)


class Span(Model):
    start: float = Field(ge=0)
    end: float = Field(gt=0)

    @model_validator(mode="after")
    def ordered(self):
        if self.end <= self.start:
            raise ValueError("end must be after start")
        return self


class Clip(Model):
    id: str = Field(default_factory=uid)
    path: str
    camera: Literal["A", "B", "master"]
    duration: float = Field(gt=0)
    channels: int = Field(default=0, ge=0)
    has_video: bool = False
    frame_rate: float | None = Field(default=None, gt=0, le=240)
    fingerprint: str
    offset: float = 0
    rate: float = Field(default=1, ge=0.98, le=1.02)
    sync_score: float | None = None
    sync_reviewed: bool = False
    sync_locked: bool = False
    warnings: list[str] = Field(default_factory=list)


class Activity(Span):
    id: str = Field(default_factory=uid)
    speaker: str


class Caption(Span):
    id: str = Field(default_factory=uid)
    speaker: str
    text: str = ""
    translation: str | None = None
    reviewed: bool = False
    locked: bool = False
    overlap: bool = False
    timing: Literal["segment", "aligned"] = "segment"


class Shot(Span):
    id: str = Field(default_factory=uid)
    layout: Literal["A", "B", "both", "black"]
    locked: bool = False
    warnings: list[str] = Field(default_factory=list)


class Cut(Span):
    id: str = Field(default_factory=uid)
    approved: bool = False


class Settings(Model):
    compute_device: Literal['auto', 'cpu', 'cuda'] = 'auto'
    transcription_segment_seconds: float = Field(default=28, ge=8, le=28)
    host_speaker: str | None = None
    roles_locked: bool = False
    minimum_shot: float = Field(default=4, ge=0, le=30)
    meaningful_overlap: float = Field(default=0.6, ge=0, le=10)
    acknowledgement: float = Field(default=0.7, ge=0, le=5)
    overlap_padding: float = Field(default=1, ge=0, le=5)
    speaker_camera: dict[str, Literal["A", "B"]] = Field(default_factory=lambda: {"SPEAKER_00": "A", "SPEAKER_01": "B"})
    speaker_channel: dict[str, int] = Field(default_factory=dict)


class Project(Model):
    schema_version: Literal[1] = 1
    id: str = Field(default_factory=uid)
    name: str = Field(min_length=1, max_length=200)
    revision: int = Field(default=0, ge=0)
    duration: float = Field(default=0, ge=0)
    clips: list[Clip] = Field(default_factory=list)
    activity: list[Activity] = Field(default_factory=list)
    captions: list[Caption] = Field(default_factory=list)
    shots: list[Shot] = Field(default_factory=list)
    cuts: list[Cut] = Field(default_factory=list)
    reactions: list['Reaction'] = Field(default_factory=list)
    settings: Settings = Field(default_factory=Settings)

    @model_validator(mode="after")
    def consistent(self):
        masters = [c for c in self.clips if c.camera == "master"]
        if self.clips and len(masters) != 1:
            raise ValueError("project needs exactly one master audio")
        if masters and abs(self.duration - masters[0].duration) > .001:
            raise ValueError("duration must match master")
        for field in (self.clips, self.activity, self.captions, self.shots, self.cuts, self.reactions):
            if len({x.id for x in field}) != len(field):
                raise ValueError("duplicate IDs")
        for span in [*self.activity, *self.captions, *self.shots, *self.cuts, *self.reactions]:
            if span.end > self.duration + .001:
                raise ValueError("span exceeds master timeline")
        for spans in (self.shots, self.cuts):
            ordered = sorted(spans, key=lambda s: s.start)
            if any(a.end > b.start + 1e-6 for a, b in zip(ordered, ordered[1:])):
                raise ValueError("shots/cuts must not overlap")
        for channel in self.settings.speaker_channel.values():
            if not masters or not 0 <= channel < masters[0].channels:
                raise ValueError("speaker channel outside master audio channels")
        kept = sorted((r for r in self.reactions if r.decision == 'keep'), key=lambda r:r.start)
        if any(a.end > b.start for a,b in zip(kept, kept[1:])):
            raise ValueError('Kept reaction suggestions must not overlap')
        if any(not any(c.id == r.clip_id and c.has_video for c in self.clips) for r in self.reactions):
            raise ValueError('Reaction must refer to a camera shard in this project')
        return self


class Reaction(Span):
    id: str = Field(default_factory=uid)
    clip_id: str
    score: float = Field(ge=0, le=1)
    cues: list[str] = Field(default_factory=list)
    decision: Literal['pending', 'keep', 'reject'] = 'pending'


Project.model_rebuild()
