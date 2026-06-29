<?php

namespace App\Controller;

use App\Entity\DemoJob;
use App\Message\DemoPingMessage;
use App\Repository\DemoJobRepository;
use Doctrine\ORM\EntityManagerInterface;
use Symfony\Bundle\FrameworkBundle\Controller\AbstractController;
use Symfony\Component\HttpFoundation\JsonResponse;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\Messenger\MessageBusInterface;
use Symfony\Component\Routing\Attribute\Route;
use Symfony\Component\Uid\Uuid;

final class DemoController extends AbstractController
{
    #[Route('/', name: 'demo_index', methods: ['GET'])]
    public function index(DemoJobRepository $jobs): Response
    {
        return $this->render('demo/index.html.twig', [
            'mercureTopic' => DemoPingMessage::TOPIC,
            'jobCount' => $jobs->count([]),
        ]);
    }

    #[Route('/api/dispatch', name: 'demo_dispatch', methods: ['POST'])]
    public function dispatch(MessageBusInterface $bus, EntityManagerInterface $em): JsonResponse
    {
        $jobId = Uuid::v7()->toRfc4122();

        // Persist the job before dispatching so it survives restarts and proves
        // the schema is live; the worker flips it to "done" when handled.
        $em->persist(new DemoJob($jobId));
        $em->flush();

        $bus->dispatch(new DemoPingMessage($jobId, time()));

        return $this->json([
            'ok' => true,
            'jobId' => $jobId,
            'serverCountIncrement' => 1,
        ]);
    }

    #[Route('/healthz', name: 'healthz', methods: ['GET'])]
    public function healthz(): JsonResponse
    {
        return $this->json(['status' => 'ok']);
    }
}
